use crate::catch_unwind::CatchUnwind;
use crate::layout_helpers::extend;
use crate::lightproc::LightProc;
use crate::proc_data::ProcData;
use crate::proc_layout::ProcLayout;
use crate::proc_vtable::ProcVTable;
use crate::state::*;
use std::alloc::{self, Layout};
use std::cell::Cell;
use std::future::Future;
use std::mem::{self, ManuallyDrop};
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::ptr::NonNull;
use std::sync::atomic::Ordering;

use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

/// Raw pointers to the fields of a proc.
pub(crate) struct RawProc<F, R, S> {
    pub(crate) pdata: *const ProcData,
    pub(crate) schedule: *const S,
    pub(crate) future: *mut F,
    pub(crate) output: *mut R,
}

impl<F, R, S> RawProc<F, R, S>
where
    F: Future<Output = R> + 'static,
    R: 'static,
    S: Fn(LightProc) + 'static,
{
    /// Allocates a proc with the given `future` and `schedule` function.
    ///
    /// It is assumed there are initially only the `LightProc` reference and the `ProcHandle`.
    pub(crate) fn allocate(future: F, schedule: S) -> NonNull<()> {
        // Compute the layout of the proc for allocation. Abort if the computation fails.
        let proc_layout = Self::proc_layout();

        unsafe {
            // Allocate enough space for the entire proc.
            let raw_proc = match NonNull::new(alloc::alloc(proc_layout.layout) as *mut ()) {
                None => std::process::abort(),
                Some(p) => p,
            };

            let raw = Self::from_ptr(raw_proc.as_ptr());


            // Write the pdata as the first field of the proc.
            (raw.pdata as *mut ProcData).write(ProcData {
                state: AtomicState::new(SCHEDULED | HANDLE | REFERENCE),
                awaiter: Cell::new(None),
                vtable: &ProcVTable {
                    raw_waker: RawWakerVTable::new(
                        Self::clone_waker,
                        Self::wake,
                        Self::wake_by_ref,
                        Self::decrement,
                    ),
                    schedule: Self::schedule,
                    drop_future: Self::drop_future,
                    get_output: Self::get_output,
                    decrement: Self::decrement,
                    destroy: Self::destroy,
                    tick: Self::tick,
                },
            });

            // Write the schedule function as the third field of the proc.
            (raw.schedule as *mut S).write(schedule);

            // Write the future as the fourth field of the proc.
            raw.future.write(future);

            raw_proc
        }
    }

    /// Returns the memory layout for a proc.
    #[inline(always)]
    fn proc_layout() -> ProcLayout {
        let layout_pdata = Layout::new::<ProcData>();
        let layout_schedule = Layout::new::<S>();
        let layout_future = Layout::new::<CatchUnwind<AssertUnwindSafe<F>>>();
        let layout_output = Layout::new::<R>();

        let size_union = layout_future.size().max(layout_output.size());
        let align_union = layout_future.align().max(layout_output.align());
        let layout_union = unsafe { Layout::from_size_align_unchecked(size_union, align_union) };

        let layout = layout_pdata;
        let (layout, offset_schedule) = extend(layout, layout_schedule);
        let (layout, offset_union) = extend(layout, layout_union);
        let offset_future = offset_union;
        let offset_output = offset_union;

        ProcLayout {
            layout,
            offset_schedule,
            offset_future,
            offset_output,
        }
    }

    /// Creates a `RawProc` from a raw proc pointer.
    #[inline]
    pub(crate) fn from_ptr(ptr: *const ()) -> Self {
        let proc_layout = Self::proc_layout();
        let p = ptr as *const u8;

        unsafe {
            Self {
                pdata: p as *const ProcData,
                schedule: p.add(proc_layout.offset_schedule) as *const S,
                future: p.add(proc_layout.offset_future) as *mut F,
                output: p.add(proc_layout.offset_output) as *mut R,
            }
        }
    }

    /// Wakes a waker.
    unsafe fn wake(ptr: *const ()) {
        let raw = Self::from_ptr(ptr);

        let mut state = (*raw.pdata).state.load(Ordering::Acquire);

        loop {
            // If the proc is completed or closed, it can't be woken.
            if state.intersects(COMPLETED | CLOSED) {
                // Drop the waker.
                Self::decrement(ptr);
                break;
            }

            // If the proc is already scheduled, we just need to synchronize with the thread that
            // will run the proc by "publishing" our current view of the memory.
            if state.is_scheduled() {
                // Update the state without actually modifying it.
                match (*raw.pdata).state.compare_exchange_weak(
                    state.into(),
                    state.into(),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {
                        // Drop the waker.
                        Self::decrement(ptr);
                        break;
                    }
                    Err(s) => state = s,
                }
            } else {
                // Mark the proc as scheduled.
                match (*raw.pdata).state.compare_exchange_weak(
                    state,
                    state | SCHEDULED,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {
                        // If the proc was not yet scheduled and isn't currently running, now is the
                        // time to schedule it.
                        if !state.is_running() {
                            // Schedule the proc.
                            let proc = LightProc {
                                raw_proc: NonNull::new_unchecked(ptr as *mut ()),
                            };
                            (*raw.schedule)(proc);
                        } else {
                            // Drop the waker.
                            Self::decrement(ptr);
                        }

                        break;
                    }
                    Err(s) => state = s,
                }
            }
        }
    }

    /// Wakes a waker by reference.
    unsafe fn wake_by_ref(ptr: *const ()) {
        let raw = Self::from_ptr(ptr);

        let mut state = (*raw.pdata).state.load(Ordering::Acquire);

        loop {
            // If the proc is completed or closed, it can't be woken.
            if state.intersects(COMPLETED | CLOSED) {
                break;
            }

            // If the proc is already scheduled, we just need to synchronize with the thread that
            // will run the proc by "publishing" our current view of the memory.
            if state.is_scheduled() {
                // Update the state without actually modifying it.
                match (*raw.pdata).state.compare_exchange_weak(
                    state,
                    state,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => break,
                    Err(s) => state = s,
                }
            } else {
                // If the proc is not scheduled nor running, we'll need to schedule after waking.
                let new = if !state.intersects(SCHEDULED | RUNNING) {
                    (state | SCHEDULED) + 1
                } else {
                    state | SCHEDULED
                };

                // Mark the proc as scheduled.
                match (*raw.pdata).state.compare_exchange_weak(
                    state,
                    new,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {
                        // If the proc is not scheduled nor running, now is the time to schedule.
                        if !state.intersects(SCHEDULED | RUNNING) {
                            // Schedule the proc.
                            let proc = LightProc {
                                raw_proc: NonNull::new_unchecked(ptr as *mut ()),
                            };
                            (*raw.schedule)(proc);
                        }

                        break;
                    }
                    Err(s) => state = s,
                }
            }
        }
    }

    /// Clones a waker.
    unsafe fn clone_waker(ptr: *const ()) -> RawWaker {
        let raw = Self::from_ptr(ptr);
        let raw_waker = &(*raw.pdata).vtable.raw_waker;

        // Increment the reference count. With any kind of reference-counted data structure,
        // relaxed ordering is fine when the reference is being cloned.
        let state = (*raw.pdata).state.fetch_add(1, Ordering::Relaxed);

        // If the reference count overflowed, abort.
        if state.bits() > i64::MAX as u64 {
            std::process::abort();
        }

        RawWaker::new(ptr, raw_waker)
    }

    /// Drops a waker or a proc.
    ///
    /// This function will decrement the reference count. If it drops down to zero and the
    /// associated join handle has been dropped too, then the proc gets destroyed.
    #[inline]
    unsafe fn decrement(ptr: *const ()) {
        let raw = Self::from_ptr(ptr);

        // Decrement the reference count.
        let mut new = (*raw.pdata)
            .state
            .fetch_sub(1, Ordering::AcqRel);
        new.set_refcount(new.get_refcount().saturating_sub(1));

        // If this was the last reference to the proc and the `ProcHandle` has been dropped as
        // well, then destroy the proc.
        if new.get_refcount() == 0 && !new.is_handle() {
            Self::destroy(ptr);
        }
    }

    /// Schedules a proc for running.
    ///
    /// This function doesn't modify the state of the proc. It only passes the proc reference to
    /// its schedule function.
    unsafe fn schedule(ptr: *const ()) {
        let raw = Self::from_ptr(ptr);

        (*raw.schedule)(LightProc {
            raw_proc: NonNull::new_unchecked(ptr as *mut ()),
        });
    }

    /// Drops the future inside a proc.
    #[inline]
    unsafe fn drop_future(ptr: *const ()) {
        let raw = Self::from_ptr(ptr);

        // We need a safeguard against panics because the destructor can panic.
        raw.future.drop_in_place();
    }

    /// Returns a pointer to the output inside a proc.
    unsafe fn get_output(ptr: *const ()) -> *const () {
        let raw = Self::from_ptr(ptr);
        raw.output as *const ()
    }

    /// Cleans up proc's resources and deallocates it.
    ///
    /// If the proc has not been closed, then its future or the output will be dropped. The
    /// schedule function gets dropped too.
    #[inline]
    unsafe fn destroy(ptr: *const ()) {
        let raw = Self::from_ptr(ptr);
        let proc_layout = Self::proc_layout();

        // We need a safeguard against panics because destructors can panic.
        // Drop the schedule function.
        (raw.schedule as *mut S).drop_in_place();

        // Finally, deallocate the memory reserved by the proc.
        alloc::dealloc(ptr as *mut u8, proc_layout.layout);
    }

    /// Ticks a proc.
    ///
    /// Ticking will call `poll` once and re-schedule the task if it returns `Poll::Pending`. If
    /// polling its future panics, the proc will be closed and the panic propagated into the caller.
    unsafe fn tick(ptr: *const ()) {
        let raw = Self::from_ptr(ptr);

        // Create a context from the raw proc pointer and the vtable inside the its pdata.
        let waker = ManuallyDrop::new(Waker::from_raw(RawWaker::new(
            ptr,
            &(*raw.pdata).vtable.raw_waker,
        )));
        let cx = &mut Context::from_waker(&waker);

        let mut state = (*raw.pdata).state.load(Ordering::Acquire);

        // Update the proc's state before polling its future.
        loop {
            // If the proc has been closed, drop the proc reference and return.
            if state.is_closed() {
                // Notify the awaiter that the proc has been closed.
                if state.is_awaiter() {
                    (*raw.pdata).notify();
                }

                // Drop the future.
                Self::drop_future(ptr);

                // Drop the proc reference.
                Self::decrement(ptr);
                return;
            }

            // Mark the proc as unscheduled and running.
            match (*raw.pdata).state.compare_exchange_weak(
                state,
                (state & !SCHEDULED) | RUNNING,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // Update the state because we're continuing with polling the future.
                    state = (state & !SCHEDULED) | RUNNING;
                    break;
                }
                Err(s) => state = s,
            }
        }
        // If we get here the lightproc is not closed and marked as running and unscheduled.

        // Poll the inner future, but surround it with a guard that closes the proc in case polling
        // panics.
        let guard = Guard(raw);
        let poll = <F as Future>::poll(Pin::new_unchecked(&mut *raw.future), cx);
        mem::forget(guard);

        match poll {
            Poll::Ready(out) => {
                // Replace the future with its output.
                Self::drop_future(ptr);
                raw.output.write(out);

                // A place where the output will be stored in case it needs to be dropped.
                let mut output = None;

                // The proc is now completed.
                loop {
                    // If the handle is dropped, we'll need to close it and drop the output.
                    let new = if !state.is_handle() {
                        (state & !RUNNING & !SCHEDULED) | COMPLETED | CLOSED
                    } else {
                        (state & !RUNNING & !SCHEDULED) | COMPLETED
                    };

                    // Mark the proc as not running and completed.
                    match (*raw.pdata).state.compare_exchange_weak(
                        state,
                        new,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    ) {
                        Ok(_) => {
                            // If the handle is dropped or if the proc was closed while running,
                            // now it's time to drop the output.
                            if !state.is_handle() || state.is_closed() {
                                // Read the output.
                                output = Some(raw.output.read());
                            }

                            // Notify the awaiter that the proc has been completed.
                            if state.is_awaiter() {
                                (*raw.pdata).notify();
                            }

                            // Drop the proc reference.
                            Self::decrement(ptr);
                            break;
                        }
                        Err(s) => state = s,
                    }
                }

                // Drop the output if it was taken out of the proc.
                drop(output);
            }
            Poll::Pending => {
                // The proc is still not completed.
                loop {
                    // If the proc was closed while running, we'll need to unschedule in case it
                    // was woken and then clean up its resources.
                    let new = if state.is_closed() {
                        state & !( RUNNING | SCHEDULED )
                    } else {
                        state & !RUNNING
                    };

                    // Mark the proc as not running.
                    match (*raw.pdata).state.compare_exchange_weak(
                        state,
                        new,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    ) {
                        Ok(state) => {
                            // If the proc was closed while running, we need to drop its future.
                            // If the proc was woken while running, we need to schedule it.
                            // Otherwise, we just drop the proc reference.
                            if state.is_closed() {
                                // The thread that closed the proc didn't drop the future because
                                // it was running so now it's our responsibility to do so.
                                Self::drop_future(ptr);

                                // Drop the proc reference.
                                Self::decrement(ptr);
                            } else if state.is_scheduled() {
                                // The thread that has woken the proc didn't reschedule it because
                                // it was running so now it's our responsibility to do so.
                                Self::schedule(ptr);
                            } else {
                                // Drop the proc reference.
                                Self::decrement(ptr);
                            }
                            break;
                        }
                        Err(s) => state = s,
                    }
                }
            }
        }
    }
}

impl<F, R, S> Clone for RawProc<F, R, S> {
    fn clone(&self) -> Self {
        Self {
            pdata: self.pdata,
            schedule: self.schedule,
            future: self.future,
            output: self.output,
        }
    }
}
impl<F, R, S> Copy for RawProc<F, R, S> {}

/// A guard that closes the proc if polling its future panics.
struct Guard<F, R, S>(RawProc<F, R, S>)
    where
        F: Future<Output = R> + 'static,
        R: 'static,
        S: Fn(LightProc) + 'static;

impl<F, R, S> Drop for Guard<F, R, S>
where
    F: Future<Output = R> + 'static,
    R: 'static,
    S: Fn(LightProc) + 'static,
{
    fn drop(&mut self) {
        let raw = self.0;
        let ptr = raw.pdata as *const ();

        unsafe {
            let mut state = (*raw.pdata).state.load(Ordering::Acquire);

            loop {
                // If the proc was closed while running, then unschedule it, drop its
                // future, and drop the proc reference.
                if state.is_closed() {
                    // We still need to unschedule the proc because it is possible it was
                    // woken while running.
                    (*raw.pdata).state.fetch_and(!SCHEDULED, Ordering::AcqRel);

                    // The thread that closed the proc didn't drop the future because it
                    // was running so now it's our responsibility to do so.
                    RawProc::<F, R, S>::drop_future(ptr);

                    // Drop the proc reference.
                    RawProc::<F, R, S>::decrement(ptr);
                    break;
                }

                // Mark the proc as not running, not scheduled, and closed.
                match (*raw.pdata).state.compare_exchange_weak(
                    state,
                    (state & !RUNNING & !SCHEDULED) | CLOSED,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(state) => {
                        // Drop the future because the proc is now closed.
                        RawProc::<F, R, S>::drop_future(ptr);

                        // Notify the awaiter that the proc has been closed.
                        if state.is_awaiter() {
                            (*raw.pdata).notify();
                        }

                        // Drop the proc reference.
                        RawProc::<F, R, S>::decrement(ptr);
                        break;
                    }
                    Err(s) => state = s,
                }
            }
        }
    }
}
