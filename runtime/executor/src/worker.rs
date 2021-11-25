use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;
use crossbeam_deque::{Injector, Steal, Stealer, Worker};
use crossbeam_queue::SegQueue;
use crossbeam_utils::sync::{Parker, Unparker};
use lightproc::prelude::LightProc;

pub trait Runnable {
    fn run(self);
}
impl Runnable for LightProc {
    fn run(self) {
        LightProc::run(self)
    }
}

#[derive(Debug)]
/// A thread worker pulling tasks from a shared injector queue and executing them
pub(crate) struct WorkerThread<'a, Task> {
    /// Shared task queue
    task_queue: Arc<Injector<Task>>,

    /// This threads task queue. For efficiency reasons worker threads pull a batch of tasks
    /// from the injector queue and work on them instead of pulling them one by one. Should the
    /// global queue become empty worker threads can steal tasks from each other.
    tasks: Worker<Task>,

    /// Queue of `!Send` tasks that have to be entirely ran on this thread and must not be moved
    /// or stolen to other threads.
    local_tasks: SegQueue<Task>,

    /// Thread parker.
    ///
    /// A worker thread will park when there is no more work it can do. Work threads can be
    /// unparked by either a local task being woken up or by the Executor owning the Injector queue.
    parker: Parker,

    _marker: PhantomData<&'a ()>,
}

#[derive(Debug)]
pub struct Sleeper<Task> {
    stealer: Stealer<Task>,
    unparker: Unparker,
}

impl<Task> Sleeper<Task> {
    pub fn wakeup(&self) {
        self.unparker.unpark();
    }
}

impl<'a, T: Runnable + 'a> WorkerThread<'a, T> {
    pub fn new(task_queue: Arc<Injector<T>>) -> (WorkerThread<'a, T>, Sleeper<T>) {
        let tasks: Worker<T> = Worker::new_fifo();
        let stealer = tasks.stealer();
        let local_tasks: SegQueue<T> = SegQueue::new();
        let parker = Parker::new();
        let _marker = PhantomData;
        let unparker = parker.unparker().clone();

        (
            Self { task_queue, tasks, local_tasks, parker, _marker },
            Sleeper { stealer, unparker }
        )
    }

    pub fn unparker(&self) -> &Unparker {
        self.parker.unparker()
    }

    /// Run this worker thread "forever" (i.e. until the thread panics or is otherwise killed)
    pub fn run(&self, fences: impl Iterator<Item=&'a Stealer<T>>) -> ! {
        let fences: Vec<Stealer<T>> = fences
            .map(|stealer| stealer.clone())
            .collect();

        loop {
            self.run_inner(&fences);
            self.parker.park();
        }
    }

    pub fn run_timeout(&self, fences: impl Iterator<Item=&'a Stealer<T>>, timeout: Duration) -> ! {
        let fences: Vec<Stealer<T>> = fences
            .map(|stealer| stealer.clone())
            .collect();

        loop {
            self.run_inner(&fences);
            self.parker.park_timeout(timeout);
        }
    }

    pub fn run_once(&self, fences: impl Iterator<Item=&'a Stealer<T>>) {
        let fences: Vec<Stealer<T>> = fences
            .map(|stealer| stealer.clone())
            .collect();

        self.run_inner(fences);
    }

    fn run_inner<F: AsRef<[Stealer<T>]>>(&self, fences: F) {
        // Continue working until there is no work to do.
        'work: while {
            // Always run local tasks first since they can't be done by anybody else.
            if let Some(task) = self.local_tasks.pop() {
                task.run();
                continue 'work;
            } else if let Some(task) = self.tasks.pop() {
                task.run();
                continue 'work;
            } else {
                // If we were woken up by the global scheduler `should_steal` is set to true,
                // so we now try to clean out.

                // First try to take work from the global queue.
                let mut i = 0;
                loop {
                    match self.task_queue.steal_batch_and_pop(&self.tasks) {
                        // If we could steal from the global queue do more work.
                        Steal::Success(task) => {
                            task.run();
                            continue 'work;
                        },

                        // If there is no more work to steal from the global queue, try other
                        // workers next
                        Steal::Empty => break,

                        // If a race condition occurred try again with backoff
                        Steal::Retry => for _ in 0..(1 << i) {
                            core::hint::spin_loop();
                            i += 1;
                        },
                    }
                }

                // If the global queue is empty too, steal from the thread with the most work.
                // This is only None when there are no stealers installed which, given that we
                // exist, *should* never be the case.
                while let Some(fence) = select_fence(fences.as_ref().iter()) {
                    match fence.steal_batch_and_pop(&self.tasks) {
                        Steal::Success(task) => {
                            task.run();
                            continue 'work;
                        },

                        // If no other worker has work to do we're done once again.
                        Steal::Empty => break,

                        // If another worker is currently stealing chances are that the
                        // current `stealer` will not have the most task afterwards so we do
                        // want to do the maths regarding that again.
                        Steal::Retry => core::hint::spin_loop(),
                    }
                }
            }

            // If we get here we're done and need to park.
            false
        } {}
    }

    pub fn schedule_local(&self, task: T) {
        self.local_tasks.push(task);
    }
}

#[inline(always)]
fn select_fence<'a, T>(fences: impl Iterator<Item=&'a Stealer<T>>) -> Option<&'a Stealer<T>> {
    fences.max_by_key(|fence| fence.len())
}