//!
//! Handle for recoverable process
use crate::proc_data::ProcData;
use crate::proc_handle::ProcHandle;
use crate::state::State;
use std::any::Any;
use std::fmt::{self, Debug, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::thread;

/// Recoverable handle which encapsulates a standard Proc Handle and contain all panics inside.
///
/// Execution of `after_panic` will be immediate on polling the [RecoverableHandle]'s future.
pub struct RecoverableHandle<R> {
    inner: ProcHandle<thread::Result<R>>,

    /// Panic callback
    ///
    /// This callback will be called if the interior future panics. It is passed the panic
    // reason i.e. the `Err` of [`std::thread::Result`]
    panicked: Option<Box<dyn FnOnce(Box<dyn Any + Send>) + Send + Sync>>,
}

impl<R> RecoverableHandle<R> {
    pub(crate) fn new(inner: ProcHandle<thread::Result<R>>) -> Self {
        RecoverableHandle {
            inner,
            panicked: None,
        }
    }

    /// Cancels the proc.
    ///
    /// If the proc has already completed, calling this method will have no effect.
    ///
    /// When a proc is cancelled, its future cannot be polled again and will be dropped instead.
    pub fn cancel(&self) {
        self.inner.cancel()
    }

    /// Returns a state of the ProcHandle.
    pub fn state(&self) -> State {
        self.inner.state()
    }

    /// Adds a callback that will be executed should the inner future `panic!`s
    ///
    /// ```rust
    /// # use std::any::Any;
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
    /// // ... creating a recoverable process
    /// let (proc, recoverable) = LightProc::recoverable(
    ///     future,
    ///     schedule_function,
    ///     Span::current(),
    ///     None
    /// );
    ///
    /// recoverable
    ///     .on_panic(|_e: Box<dyn Any + Send>| {
    ///         println!("Inner future panicked");
    ///     });
    /// ```
    pub fn on_panic<F>(mut self, callback: F) -> Self
    where
        F: FnOnce(Box<dyn Any + Send>) + Send + Sync + 'static,
    {
        self.panicked = Some(Box::new(callback));
        self
    }
}

impl<R> Future for RecoverableHandle<R> {
    type Output = Option<R>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        match Pin::new(&mut self.inner).poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Ready(Some(Ok(val))) => Poll::Ready(Some(val)),
            Poll::Ready(Some(Err(e))) => {
                if let Some(callback) = self.panicked.take() {
                    callback(e);
                }

                Poll::Ready(None)
            }
        }
    }
}

impl<R> Debug for RecoverableHandle<R> {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        let ptr = self.inner.raw_proc.as_ptr();
        let pdata = ptr as *const ProcData;

        fmt.debug_struct("ProcHandle")
            .field("pdata", unsafe { &(*pdata) })
            .finish_non_exhaustive()
    }
}
