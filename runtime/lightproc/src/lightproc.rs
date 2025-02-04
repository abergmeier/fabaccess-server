//!
//! Lightweight process implementation which enables users
//! to create either panic recoverable process or
//! ordinary process.
//!
//! Lightweight processes needs a stack to use their lifecycle
//! operations like `before_start`, `after_complete` and more...
//!
//! # Example Usage
//!
//! ```rust
//! use tracing::Span;
//! use lightproc::prelude::*;
//!
//! // ... future that does work
//! let future = async {
//!     println!("Doing some work");
//! };
//!
//! // ... basic schedule function with no waker logic
//! fn schedule_function(proc: LightProc) {;}
//!
//! // ... creating a recoverable process
//! let panic_recoverable = LightProc::recoverable(
//!     future,
//!     schedule_function,
//!     Span::current(),
//!     None,
//! );
//! ```

use crate::proc_data::ProcData;
use crate::proc_ext::ProcFutureExt;
use crate::proc_handle::ProcHandle;
use crate::raw_proc::RawProc;
use crate::recoverable_handle::RecoverableHandle;
use crate::GroupId;
use std::fmt::{self, Debug, Formatter};
use std::future::Future;
use std::mem::ManuallyDrop;
use std::panic::AssertUnwindSafe;
use std::ptr::NonNull;
use tracing::Span;

/// Shared functionality for both Send and !Send LightProc
pub struct LightProc {
    /// A pointer to the heap-allocated proc.
    pub(crate) raw_proc: NonNull<()>,
}

// LightProc is both Sync and Send because it explicitly handles synchronization internally:
// The state of a `LightProc` is only modified atomically guaranteeing a consistent view from all
// threads. Existing wakers (and the proc_handle) are atomically reference counted so the proc
// itself will not be dropped until all pointers to it are themselves dropped.
// However, if the future or result inside the LightProc is !Send the executor must ensure that
// the `schedule` function does not move the LightProc to a different thread.
unsafe impl Send for LightProc {}
unsafe impl Sync for LightProc {}

impl LightProc {
    /// Creates a recoverable process which will catch panics in the given future.
    ///
    /// # Example
    /// ```rust
    /// # use std::any::Any;
    /// # use tracing::Span;
    /// # use lightproc::prelude::*;
    /// #
    /// # // ... basic schedule function with no waker logic
    /// # fn schedule_function(proc: LightProc) {;}
    /// #
    /// let future = async {
    ///     panic!("oh no!");
    /// };
    /// // ... creating a recoverable process
    /// let (proc, handle) = LightProc::recoverable(
    ///     future,
    ///     schedule_function,
    ///     Span::current(),
    ///     None
    /// );
    /// let handle = handle.on_panic(|e: Box<dyn Any + Send>| {
    ///     let reason = e.downcast::<String>().unwrap();
    ///     println!("future panicked!: {}", &reason);
    /// });
    /// ```
    pub fn recoverable<'a, F, R, S>(
        future: F,
        schedule: S,
        span: Span,
        cgroup: Option<GroupId>,
    ) -> (Self, RecoverableHandle<R>)
    where
        F: Future<Output = R> + 'a,
        R: 'a,
        S: Fn(LightProc) + 'a,
    {
        let recovery_future = AssertUnwindSafe(future).catch_unwind();
        let (proc, handle) = Self::build(recovery_future, schedule, span, cgroup);
        (proc, RecoverableHandle::new(handle))
    }

    ///
    /// Creates a process which will stop its execution on occurrence of panic.
    ///
    /// # Example
    /// ```rust
    /// # use tracing::Span;
    /// # use lightproc::prelude::*;
    /// #
    /// # // ... future that does work
    /// # let future = async {
    /// #     println!("Doing some work");
    /// # };
    /// #
    /// # // ... basic schedule function with no waker logic
    /// # fn schedule_function(proc: LightProc) {;}
    /// #
    /// // ... creating a standard process
    /// let standard = LightProc::build(
    ///     future,
    ///     schedule_function,
    ///     Span::current(),
    ///     None,
    /// );
    /// ```
    pub fn build<'a, F, R, S>(
        future: F,
        schedule: S,
        span: Span,
        cgroup: Option<GroupId>,
    ) -> (Self, ProcHandle<R>)
    where
        F: Future<Output = R> + 'a,
        R: 'a,
        S: Fn(LightProc) + 'a,
    {
        let raw_proc = RawProc::allocate(future, schedule, span, cgroup);
        let proc = LightProc { raw_proc };
        let handle = ProcHandle::new(raw_proc);
        (proc, handle)
    }

    ///
    /// Schedule the lightweight process with passed `schedule` function at the build time.
    pub fn schedule(self) {
        let this = ManuallyDrop::new(self);
        let ptr = this.raw_proc.as_ptr();
        let pdata = ptr as *const ProcData;

        unsafe {
            ((*pdata).vtable.schedule)(ptr);
        }
    }

    /// Run this LightProc.
    ///
    /// "Running" a lightproc means ticking it once and if it doesn't complete
    /// immediately re-scheduling it as soon as it's Waker wakes it back up.
    pub fn run(self) {
        let this = ManuallyDrop::new(self);
        let ptr = this.raw_proc.as_ptr();
        let pdata = ptr as *const ProcData;

        unsafe {
            ((*pdata).vtable.tick)(ptr);
        }
    }

    /// Cancel polling the lightproc's inner future, thus cancelling the proc itself.
    pub fn cancel(&self) {
        let ptr = self.raw_proc.as_ptr();
        let pdata = ptr as *const ProcData;

        unsafe {
            (*pdata).cancel();
        }
    }
}

impl Debug for LightProc {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        let ptr = self.raw_proc.as_ptr();
        let pdata = ptr as *const ProcData;

        fmt.debug_struct("LightProc")
            .field("pdata", unsafe { &(*pdata) })
            .finish()
    }
}

impl Drop for LightProc {
    fn drop(&mut self) {
        let ptr = self.raw_proc.as_ptr();
        let pdata = ptr as *const ProcData;

        unsafe {
            // Cancel the proc.
            (*pdata).cancel();

            // Drop the future.
            ((*pdata).vtable.drop_future)(ptr);

            // Drop the proc reference.
            ((*pdata).vtable.decrement)(ptr);
        }
    }
}
