//!
//! Handle for tasks which don't need to unwind panics inside
//! the given futures.
use crate::proc_data::ProcData;
use crate::state::*;
use std::fmt::{self, Debug, Formatter};
use std::future::Future;
use std::marker::{PhantomData, Unpin};
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::ptr::NonNull;
use std::sync::atomic::Ordering;
use std::task::{Context, Poll};

/// A handle that awaits the result of a proc.
///
/// This type is a future that resolves to an `Option<R>` where:
///
/// * `None` indicates the proc has panicked or was cancelled
/// * `Some(res)` indicates the proc has completed with `res`
pub struct ProcHandle<R> {
    /// A raw proc pointer.
    pub(crate) raw_proc: NonNull<()>,

    /// A marker capturing the generic type `R`.
    // TODO: Instead of writing the future output to the RawProc on heap, put it in the handle
    //       (if still available).
    pub(crate) result: MaybeUninit<R>,
}

unsafe impl<R: Send> Send for ProcHandle<R> {}
unsafe impl<R: Sync> Sync for ProcHandle<R> {}

impl<R> Unpin for ProcHandle<R> {}

impl<R> ProcHandle<R> {
    pub(crate) fn new(raw_proc: NonNull<()>) -> Self {
        Self {
            raw_proc,
            result: MaybeUninit::uninit(),
        }
    }

    /// Cancels the proc.
    ///
    /// If the proc has already completed, calling this method will have no effect.
    ///
    /// When a proc is cancelled, its future cannot be polled again and will be dropped instead.
    pub fn cancel(&self) {
        let ptr = self.raw_proc.as_ptr();
        let pdata = ptr as *const ProcData;

        unsafe {
            let id = (&(*pdata).span).id().map(|id| id.into_u64()).unwrap_or(0);
            tracing::trace!(
                target: "executor::handle",
                op = "handle.cancel",
                task.id = id,
            );

            let mut state = (*pdata).state.load(Ordering::Acquire);

            loop {
                // If the proc has been completed or closed, it can't be cancelled.
                if state.get_flags().intersects(COMPLETED | CLOSED) {
                    break;
                }

                // If the proc is not scheduled nor running, we'll need to schedule it.
                let (flags, references) = state.parts();
                let new = if flags.intersects(SCHEDULED | RUNNING) {
                    State::new(flags | SCHEDULED | CLOSED, references + 1)
                } else {
                    State::new(flags | CLOSED, references)
                };

                // Mark the proc as closed.
                match (*pdata).state.compare_exchange_weak(
                    state,
                    new,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {
                        // If the proc is not scheduled nor running, schedule it so that its future
                        // gets dropped by the executor.
                        if !state.get_flags().intersects(SCHEDULED | RUNNING) {
                            ((*pdata).vtable.schedule)(ptr);
                        }

                        // Notify the awaiter that the proc has been closed.
                        if state.is_awaiter() {
                            (*pdata).notify();
                        }

                        break;
                    }
                    Err(s) => state = s,
                }
            }
        }
    }

    /// Returns current state of the handle.
    pub fn state(&self) -> State {
        let ptr = self.raw_proc.as_ptr();
        let pdata = ptr as *const ProcData;
        unsafe { (*pdata).state.load(Ordering::SeqCst) }
    }
}

impl<R> Future for ProcHandle<R> {
    type Output = Option<R>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let ptr = self.raw_proc.as_ptr();
        let pdata = ptr as *const ProcData;

        unsafe {
            let mut state = (*pdata).state.load(Ordering::Acquire);

            loop {
                // If the proc has been closed, notify the awaiter and return `None`.
                if state.is_closed() {
                    // Even though the awaiter is most likely the current proc, it could also be
                    // another proc.
                    (*pdata).notify_unless(cx.waker());
                    return Poll::Ready(None);
                }

                // If the proc is not completed, register the current proc.
                if !state.is_completed() {
                    // Replace the waker with one associated with the current proc. We need a
                    // safeguard against panics because dropping the previous waker can panic.
                    (*pdata).swap_awaiter(Some(cx.waker().clone()));

                    // Reload the state after registering. It is possible that the proc became
                    // completed or closed just before registration so we need to check for that.
                    state = (*pdata).state.load(Ordering::Acquire);

                    // If the proc has been closed, notify the awaiter and return `None`.
                    if state.is_closed() {
                        // Even though the awaiter is most likely the current proc, it could also
                        // be another proc.
                        (*pdata).notify_unless(cx.waker());
                        return Poll::Ready(None);
                    }

                    // If the proc is still not completed, we're blocked on it.
                    if !state.is_completed() {
                        return Poll::Pending;
                    }
                }

                // Since the proc is now completed, mark it as closed in order to grab its output.
                let (flags, references) = state.parts();
                let new = State::new(flags | CLOSED, references);
                match (*pdata).state.compare_exchange(
                    state,
                    new,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {
                        // Notify the awaiter. Even though the awaiter is most likely the current
                        // proc, it could also be another proc.
                        if state.is_awaiter() {
                            (*pdata).notify_unless(cx.waker());
                        }

                        // Take the output from the proc.
                        let output = ((*pdata).vtable.get_output)(ptr) as *mut R;
                        return Poll::Ready(Some(output.read()));
                    }
                    Err(s) => state = s,
                }
            }
        }
    }
}

impl<R> Debug for ProcHandle<R> {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        let ptr = self.raw_proc.as_ptr();
        let pdata = ptr as *const ProcData;

        fmt.debug_struct("ProcHandle")
            .field("pdata", unsafe { &(*pdata) })
            .finish_non_exhaustive()
    }
}

impl<R> Drop for ProcHandle<R> {
    fn drop(&mut self) {
        let ptr = self.raw_proc.as_ptr();
        let pdata = ptr as *const ProcData;

        // A place where the output will be stored in case it needs to be dropped.
        let mut output = None;

        unsafe {
            // Record dropping the handle for this task
            let id = (&(*pdata).span).id().map(|id| id.into_u64()).unwrap_or(0);
            tracing::trace!(
                target: "executor::handle",
                op = "handle.drop",
                task.id = id,
            );

            // Optimistically assume the `ProcHandle` is being dropped just after creating the
            // proc. This is a common case so if the handle is not used, the overhead of it is only
            // one compare-exchange operation.
            if let Err(mut state) = (*pdata).state.compare_exchange_weak(
                State::new(SCHEDULED | HANDLE, 1),
                State::new(SCHEDULED, 1),
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                loop {
                    // If the proc has been completed but not yet closed, that means its output
                    // must be dropped.
                    if state.is_completed() && !state.is_closed() {
                        // Mark the proc as closed in order to grab its output.
                        let (flags, references) = state.parts();
                        match (*pdata).state.compare_exchange_weak(
                            state,
                            State::new(flags | CLOSED, references),
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        ) {
                            Ok(_) => {
                                // Read the output.
                                output = Some((((*pdata).vtable.get_output)(ptr) as *mut R).read());

                                // Update the state variable because we're continuing the loop.
                                state = State::new(flags | CLOSED, references);
                            }
                            Err(s) => state = s,
                        }
                    } else {
                        // If this is the last reference to the proc and it's not closed, then
                        // close it and schedule one more time so that its future gets dropped by
                        // the executor.
                        let new = if state.get_refcount() == 0 && !state.is_closed() {
                            State::new(SCHEDULED | CLOSED, 1)
                        } else {
                            let (flags, references) = state.parts();
                            State::new(flags & !HANDLE, references)
                        };

                        // Unset the handle flag.
                        match (*pdata).state.compare_exchange_weak(
                            state,
                            new,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        ) {
                            Ok(_) => {
                                // If this is the last reference to the proc, we need to either
                                // schedule dropping its future or destroy it.
                                if state.get_refcount() == 0 {
                                    if !state.is_closed() {
                                        ((*pdata).vtable.schedule)(ptr);
                                    } else {
                                        ((*pdata).vtable.destroy)(ptr);
                                    }
                                }

                                break;
                            }
                            Err(s) => state = s,
                        }
                    }
                }
            }
        }

        // Drop the output if it was taken out of the proc.
        drop(output);
    }
}
