//!
//! Blocking run of the async processes
//!
//!
use crate::worker;
use crossbeam_utils::sync::{Parker, Unparker};
use std::cell::Cell;
use std::future::Future;
use std::mem;
use std::mem::{ManuallyDrop, MaybeUninit};
use std::pin::Pin;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use crate::proc_stack::ProcStack;

///
/// This method blocks the current thread until passed future is resolved with an output.
///
/// It is called `block_on` or `blocking` in some executors.
///
/// # Example
/// ```rust
/// use executor::prelude::*;
/// use lightproc::prelude::*;
/// let mut sum = 0;
///
/// run(
///     async {
///         (0..10_000_000).for_each(|_| {
///             sum += 1;
///         });
///     },
///     ProcStack::default(),
/// );
/// ```
pub fn run<F, T>(future: F, stack: ProcStack) -> T
where
    F: Future<Output = T>,
{
    unsafe {
        // An explicitly uninitialized `T`. Until `assume_init` is called this will not call any
        // drop code for T
        let mut out = MaybeUninit::uninit();

        // Wrap the future into one that stores the result into `out`.
        let future = {
            let out = out.as_mut_ptr();

            async move {
                *out = future.await;
            }
        };

        // Pin the future onto the stack.
        pin_utils::pin_mut!(future);

        // Extend the lifetime of the future to 'static.
        let future = mem::transmute::<
            Pin<&'_ mut dyn Future<Output = ()>>,
            Pin<&'static mut dyn Future<Output = ()>>,
        >(future);

        // Block on the future and and wait for it to complete.
        worker::set_stack(&stack, || block(future));

        // Assume that if the future completed and didn't panic it fully initialized its output
        out.assume_init()
    }
}

fn block<F, T>(f: F) -> T
where
    F: Future<Output = T>,
{
    thread_local! {
        // May hold a pre-allocated parker that can be reused for efficiency.
        //
        // Note that each invocation of `block` needs its own parker. In particular, if `block`
        // recursively calls itself, we must make sure that each recursive call uses a distinct
        // parker instance.
        static CACHE: Cell<Option<Parker>> = Cell::new(None);
    }

    pin_utils::pin_mut!(f);

    CACHE.with(|cache| {
        // Reuse a cached parker or create a new one for this invocation of `block`.
        let parker: Parker = cache.take().unwrap_or_else(|| Parker::new());

        let ptr = Unparker::into_raw(parker.unparker().clone());
        let vt = vtable();

        // Waker must not be dropped until it's no longer required. We also happen to know that a
        // Parker contains at least one reference to `Unparker` so the relevant `Unparker` will not
        // be dropped at least until the `Parker` is.
        let waker = unsafe { Waker::from_raw(RawWaker::new(ptr, vt)) };
        let cx = &mut Context::from_waker(&waker);

        loop {
            if let Poll::Ready(t) = f.as_mut().poll(cx) {
                // Save the parker for the next invocation of `block`.
                cache.set(Some(parker));
                return t;
            }
            parker.park();
        }
    })
}

fn vtable() -> &'static RawWakerVTable {
    /// This function will be called when the RawWaker gets cloned, e.g. when the Waker in which
    /// the RawWaker is stored gets cloned.
    //
    /// The implementation of this function must retain all resources that are required for this
    /// additional instance of a RawWaker and associated task. Calling wake on the resulting
    /// RawWaker should result in a wakeup of the same task that would have been awoken by the
    /// original RawWaker.
    unsafe fn clone_raw(ptr: *const ()) -> RawWaker {
        // [`Unparker`] implements `Clone` and upholds the contract stated above. The current
        // Implementation is simply an Arc over the actual inner values.
        let unparker = Unparker::from_raw(ptr).clone();
        RawWaker::new(Unparker::into_raw(unparker), vtable())
    }

    /// This function will be called when wake is called on the Waker. It must wake up the task
    /// associated with this RawWaker.
    ///
    /// The implementation of this function must make sure to release any resources that are
    /// associated with this instance of a RawWaker and associated task.
    unsafe fn wake_raw(ptr: *const ()) {
        // We reconstruct the Unparker from the pointer here thus ensuring it is dropped at the
        // end of this function call.
        Unparker::from_raw(ptr).unpark();
    }

    /// This function will be called when wake_by_ref is called on the Waker. It must wake up the
    /// task associated with this RawWaker.
    ///
    /// This function is similar to wake, but must not consume the provided data pointer.
    unsafe fn wake_by_ref_raw(ptr: *const ()) {
        // We **must not** drop the resulting Unparker so we wrap it in `ManuallyDrop`.
        let unparker = ManuallyDrop::new(Unparker::from_raw(ptr));
        unparker.unpark();
    }

    /// This function gets called when a RawWaker gets dropped.
    ///
    /// The implementation of this function must make sure to release any resources that are
    /// associated with this instance of a RawWaker and associated task.
    unsafe fn drop_raw(ptr: *const ()) {
        drop(Unparker::from_raw(ptr))
    }

    &RawWakerVTable::new(clone_raw, wake_raw, wake_by_ref_raw, drop_raw)
}
