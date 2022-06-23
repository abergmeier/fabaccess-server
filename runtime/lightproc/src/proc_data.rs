use crate::proc_vtable::ProcVTable;
use crate::state::*;
use crossbeam_utils::Backoff;
use std::cell::Cell;
use std::fmt::{self, Debug, Formatter};
use std::num::NonZeroU64;
use std::sync::atomic::Ordering;
use std::task::Waker;
use tracing::Span;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// Opaque id of the group this proc belongs to
pub struct GroupId(NonZeroU64);

/// The pdata of a proc.
///
/// This pdata is stored right at the beginning of every heap-allocated proc.
pub(crate) struct ProcData {
    /// Current state of the proc.
    ///
    /// Contains flags representing the current state and the reference count.
    pub(crate) state: AtomicState,

    /// The proc that is blocked on the `ProcHandle`.
    ///
    /// This waker needs to be woken once the proc completes or is closed.
    pub(crate) awaiter: Cell<Option<Waker>>,

    /// The virtual table.
    ///
    /// In addition to the actual waker virtual table, it also contains pointers to several other
    /// methods necessary for bookkeeping the heap-allocated proc.
    pub(crate) vtable: &'static ProcVTable,

    /// The span assigned to this process.
    ///
    /// A lightproc has a tracing span associated that allow recording occurances of vtable calls
    /// for this process.
    pub(crate) span: Span,

    /// Control group assigned to this process.
    ///
    /// The control group links this process to its supervision tree
    pub(crate) cgroup: Option<GroupId>,
}

impl ProcData {
    /// Cancels the proc.
    ///
    /// This method will only mark the proc as closed and will notify the awaiter, but it won't
    /// reschedule the proc if it's not completed.
    pub(crate) fn cancel(&self) {
        let mut state = self.state.load(Ordering::Acquire);

        loop {
            // If the proc has been completed or closed, it can't be cancelled.
            if state.get_flags().intersects(COMPLETED | CLOSED) {
                break;
            }

            let (flags, references) = state.parts();
            let new = State::new(flags | CLOSED, references);
            // Mark the proc as closed.
            match self
                .state
                .compare_exchange_weak(state, new, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => {
                    // Notify the awaiter that the proc has been closed.
                    if state.is_awaiter() {
                        self.notify();
                    }

                    break;
                }
                Err(s) => state = s,
            }
        }
    }

    /// Notifies the proc blocked on the proc.
    ///
    /// If there is a registered waker, it will be removed from the pdata and woken.
    #[inline]
    pub(crate) fn notify(&self) {
        if let Some(waker) = self.swap_awaiter(None) {
            // We need a safeguard against panics because waking can panic.
            waker.wake();
        }
    }

    /// Notifies the proc blocked on the proc unless its waker matches `current`.
    ///
    /// If there is a registered waker, it will be removed from the pdata.
    #[inline]
    pub(crate) fn notify_unless(&self, current: &Waker) {
        if let Some(waker) = self.swap_awaiter(None) {
            if !waker.will_wake(current) {
                // We need a safeguard against panics because waking can panic.
                waker.wake();
            }
        }
    }

    /// Swaps the awaiter and returns the previous value.
    #[inline]
    pub(crate) fn swap_awaiter(&self, new: Option<Waker>) -> Option<Waker> {
        let new_is_none = new.is_none();

        // We're about to try acquiring the lock in a loop. If it's already being held by another
        // thread, we'll have to spin for a while so it's best to employ a backoff strategy.
        let backoff = Backoff::new();
        loop {
            // Acquire the lock. If we're storing an awaiter, then also set the awaiter flag.
            let state = if new_is_none {
                self.state.fetch_or(LOCKED, Ordering::Acquire)
            } else {
                self.state.fetch_or(LOCKED | AWAITER, Ordering::Acquire)
            };

            // If the lock was acquired, break from the loop.
            if state.is_locked() {
                break;
            }

            // Snooze for a little while because the lock is held by another thread.
            backoff.snooze();
        }

        // Replace the awaiter.
        let old = self.awaiter.replace(new);

        // Release the lock. If we've cleared the awaiter, then also unset the awaiter flag.
        if new_is_none {
            self.state
                .fetch_and((!LOCKED & !AWAITER).into(), Ordering::Release);
        } else {
            self.state.fetch_and((!LOCKED).into(), Ordering::Release);
        }

        old
    }
}

impl Debug for ProcData {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        let state = self.state.load(Ordering::SeqCst);

        if fmt.alternate() {
            fmt.debug_struct("ProcData")
                .field("scheduled", &state.is_scheduled())
                .field("running", &state.is_running())
                .field("completed", &state.is_completed())
                .field("closed", &state.is_closed())
                .field("handle", &state.is_handle())
                .field("awaiter", &state.is_awaiter())
                .field("locked", &state.is_locked())
                .field("ref_count", &state.get_refcount())
                .finish()
        } else {
            fmt.debug_struct("ProcData").field("state", &state).finish()
        }
    }
}
