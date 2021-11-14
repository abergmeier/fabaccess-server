//!
//! SMP parallelism based cache affine worker implementation
//!
//! This worker implementation relies on worker run queue statistics which are hold in the pinned global memory
//! where workload distribution calculated and amended to their own local queues.

use crate::pool;

use lightproc::prelude::*;
use std::cell::Cell;
use std::ptr;
use std::time::Duration;
use crossbeam_deque::{Stealer, Worker};
use crate::proc_stack::ProcStack;

/// The timeout we'll use when parking before an other Steal attempt
pub const THREAD_PARK_TIMEOUT: Duration = Duration::from_millis(1);

thread_local! {
    static STACK: Cell<*const ProcStack> = Cell::new(ptr::null_mut());
}

///
/// Set the current process's stack during the run of the future.
pub(crate) fn set_stack<F, R>(stack: *const ProcStack, f: F) -> R
where
    F: FnOnce() -> R,
{
    struct ResetStack<'a>(&'a Cell<*const ProcStack>);

    impl Drop for ResetStack<'_> {
        fn drop(&mut self) {
            self.0.set(ptr::null());
        }
    }

    STACK.with(|st| {
        st.set(stack);
        // create a guard to reset STACK even if the future panics. This is important since we
        // must not drop the pointed-to ProcStack here in any case.
        let _guard = ResetStack(st);

        f()
    })
}

/*
pub(crate) fn get_proc_stack<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&ProcStack) -> R,
{
    let res = STACK.try_with(|st| unsafe { st.get().as_ref().map(f) });

    match res {
        Ok(Some(val)) => Some(val),
        Ok(None) | Err(_) => None,
    }
}

///
/// Get the stack currently in use for this thread
pub fn current() -> ProcStack {
    get_proc_stack(|proc| proc.clone())
        .expect("`proc::current()` called outside the context of the proc")
}
 */

pub(crate) fn schedule(proc: LightProc) {
    pool::schedule(proc)
}

/// A worker thread running futures locally and stealing work from other workers if it runs empty.
pub struct WorkerThread {
    queue: Worker<LightProc>,
}

impl WorkerThread {
    pub fn new() -> Self {
        Self {
            queue: Worker::new_fifo(),
        }
    }

    pub fn stealer(&self) -> Stealer<LightProc> {
        self.queue.stealer()
    }

    pub fn tick(&self) {
        if let Some(lightproc) =  self.queue.pop() {
            lightproc.run()
        }
    }
}