//!
//! Pool of threads to run lightweight processes
//!
//! We spawn futures onto the pool with [`spawn`] method of global run queue or
//! with corresponding [`Worker`]'s spawn method.
//!
//! [`spawn`]: crate::pool::spawn
//! [`Worker`]: crate::run_queue::Worker

use crate::run::block;
use crate::thread_manager::{DynamicRunner, ThreadManager};
use crate::worker::{Sleeper, WorkerThread};
use crossbeam_deque::{Injector, Stealer};
use lightproc::lightproc::LightProc;
use lightproc::recoverable_handle::RecoverableHandle;
use std::cell::Cell;
use std::future::Future;
use std::iter::Iterator;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::sync::Arc;
use std::time::Duration;
use tracing::field::FieldSet;
use tracing::metadata::Kind;
use tracing::{Instrument, Level, Span};

#[derive(Debug)]
struct Spooler<'a> {
    pub spool: Arc<Injector<LightProc>>,
    threads: &'a ThreadManager<AsyncRunner>,
    _marker: PhantomData<&'a ()>,
}

impl Spooler<'_> {
    pub fn new() -> Self {
        let spool = Arc::new(Injector::new());
        let threads = Box::leak(Box::new(ThreadManager::new(2, AsyncRunner, spool.clone())));
        threads.initialize();
        Self {
            spool,
            threads,
            _marker: PhantomData,
        }
    }
}

#[derive(Clone, Debug)]
/// Global executor
pub struct Executor<'a> {
    spooler: Arc<Spooler<'a>>,
}

impl<'a, 'executor: 'a> Executor<'executor> {
    pub fn new() -> Self {
        Executor {
            spooler: Arc::new(Spooler::new()),
        }
    }

    fn schedule(&self) -> impl Fn(LightProc) + 'a {
        let task_queue = self.spooler.spool.clone();
        move |lightproc: LightProc| task_queue.push(lightproc)
    }

    ///
    /// Spawn a process (which contains future + process stack) onto the executor from the global level.
    ///
    /// # Example
    /// ```rust
    /// use executor::prelude::*;
    ///
    /// # #[cfg(feature = "tokio-runtime")]
    /// # #[tokio::main]
    /// # async fn main() {
    /// #    start();
    /// # }
    /// #
    /// # #[cfg(not(feature = "tokio-runtime"))]
    /// # fn main() {
    /// #    start();
    /// # }
    /// #
    /// # fn start() {
    ///
    /// let executor = Spooler::new();
    ///
    /// let handle = executor.spawn(
    ///     async {
    ///         panic!("test");
    ///     },
    /// );
    ///
    /// executor.run(
    ///     async {
    ///         handle.await;
    ///     }
    /// );
    /// # }
    /// ```
    #[track_caller]
    pub fn spawn<F, R>(&self, future: F) -> RecoverableHandle<R>
    where
        F: Future<Output = R> + Send + 'a,
        R: Send + 'a,
    {
        let location = std::panic::Location::caller();
        let span = tracing::trace_span!(
            target: "executor::task",
            "runtime.spawn",
            loc.file = location.file(),
            loc.line = location.line(),
            loc.col = location.column(),
            kind = "global",
        );

        let (task, handle) = LightProc::recoverable(future, self.schedule(), span);
        tracing::trace!("spawning sendable task");
        task.schedule();
        handle
    }

    #[track_caller]
    pub fn spawn_local<F, R>(&self, future: F) -> RecoverableHandle<R>
    where
        F: Future<Output = R> + 'a,
        R: Send + 'a,
    {
        let location = std::panic::Location::caller();
        let span = tracing::trace_span!(
            target: "executor::task",
            "runtime.spawn",
            loc.file = location.file(),
            loc.line = location.line(),
            loc.col = location.column(),
            kind = "local",
        );

        let (task, handle) = LightProc::recoverable(future, schedule_local(), span);
        tracing::trace!("spawning sendable task");
        task.schedule();
        handle
    }

    /// Block the calling thread until the given future completes.
    ///
    /// # Example
    /// ```rust
    /// use executor::prelude::*;
    /// use lightproc::prelude::*;
    ///
    /// let executor = Spooler::new();
    ///
    /// let mut sum = 0;
    ///
    /// executor.run(
    ///     async {
    ///         (0..10_000_000).for_each(|_| {
    ///             sum += 1;
    ///         });
    ///     }
    /// );
    /// ```
    pub fn run<F, R>(&self, future: F) -> R
    where
        F: Future<Output = R>,
    {
        unsafe {
            // An explicitly uninitialized `R`. Until `assume_init` is called this will not call any
            // drop code for R
            let mut out = MaybeUninit::uninit();

            // Wrap the future into one that stores the result into `out`.
            let future = {
                let out: *mut R = out.as_mut_ptr();
                async move {
                    out.write(future.await);
                }
            };

            // Pin the future onto the stack.
            pin_utils::pin_mut!(future);

            // Block on the future and and wait for it to complete.
            block(future);

            // Assume that if the future completed and didn't panic it fully initialized its output
            out.assume_init()
        }
    }
}

#[derive(Debug)]
struct AsyncRunner;

impl DynamicRunner for AsyncRunner {
    fn setup(task_queue: Arc<Injector<LightProc>>) -> Sleeper<LightProc> {
        let (worker, sleeper) = WorkerThread::new(task_queue);
        install_worker(worker);

        sleeper
    }

    fn run_static<'b>(
        fences: impl Iterator<Item = &'b Stealer<LightProc>>,
        park_timeout: Duration,
    ) -> ! {
        let worker = get_worker();
        worker.run_timeout(fences, park_timeout)
    }

    fn run_dynamic<'b>(fences: impl Iterator<Item = &'b Stealer<LightProc>>) -> ! {
        let worker = get_worker();
        worker.run(fences)
    }

    fn run_standalone<'b>(fences: impl Iterator<Item = &'b Stealer<LightProc>>) {
        let worker = get_worker();
        worker.run_once(fences)
    }
}

thread_local! {
    static WORKER: Cell<Option<WorkerThread<'static, LightProc>>> = Cell::new(None);
}

fn get_worker() -> &'static WorkerThread<'static, LightProc> {
    WORKER.with(|cell| {
        let worker = unsafe { &*cell.as_ptr() as &'static Option<WorkerThread<_>> };
        worker
            .as_ref()
            .expect("AsyncRunner running outside Executor context")
    })
}

fn install_worker(worker_thread: WorkerThread<'static, LightProc>) {
    WORKER.with(|cell| {
        cell.replace(Some(worker_thread));
    });
}

fn schedule_local() -> impl Fn(LightProc) {
    let worker = get_worker();
    let unparker = worker.unparker().clone();
    move |lightproc| {
        // This is safe because we never replace the value in that Cell and thus never drop the
        // SharedWorker pointed to.
        worker.schedule_local(lightproc);
        // We have to unpark the worker thread for our task to be run.
        unparker.unpark();
    }
}
