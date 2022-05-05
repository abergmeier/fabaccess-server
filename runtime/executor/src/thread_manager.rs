//! A thread manager to predict how many threads should be spawned to handle the upcoming load.
//!
//! The thread manager consists of three elements:
//! * Frequency Detector
//! * Trend Estimator
//! * Predictive Upscaler
//!
//! ## Frequency Detector
//! Detects how many tasks are submitted from scheduler to thread pool in a given time frame.
//! Pool manager thread does this sampling every 90 milliseconds.
//! This value is going to be used for trend estimation phase.
//!
//! ## Trend Estimator
//! Hold up to the given number of frequencies to create an estimation.
//! Trend estimator holds 10 frequencies at a time.
//! This value is stored as constant in [FREQUENCY_QUEUE_SIZE](constant.FREQUENCY_QUEUE_SIZE.html).
//! Estimation algorithm and prediction uses Exponentially Weighted Moving Average algorithm.
//!
//! This algorithm is adapted from [A Novel Predictive and Self–Adaptive Dynamic Thread Pool Management](https://doi.org/10.1109/ISPA.2011.61)
//! and altered to:
//! * use instead of heavy calculation of trend, utilize thread redundancy which is the sum of the differences between the predicted and observed value.
//! * use instead of linear trend estimation, it uses exponential trend estimation where formula is:
//! ```text
//! LOW_WATERMARK * (predicted - observed) + LOW_WATERMARK
//! ```
//! *NOTE:* If this algorithm wants to be tweaked increasing [LOW_WATERMARK](constant.LOW_WATERMARK.html) will automatically adapt the additional dynamic thread spawn count
//! * operate without watermarking by timestamps (in paper which is used to measure algorithms own performance during the execution)
//! * operate extensive subsampling. Extensive subsampling congests the pool manager thread.
//! * operate without keeping track of idle time of threads or job out queue like TEMA and FOPS implementations.
//!
//! ## Predictive Upscaler
//! Upscaler has three cases (also can be seen in paper):
//! * The rate slightly increases and there are many idle threads.
//! * The number of worker threads tends to be reduced since the workload of the system is descending.
//! * The system has no request or stalled. (Our case here is when the current tasks block further tasks from being processed – throughput hogs)
//!
//! For the first two EMA calculation and exponential trend estimation gives good performance.
//! For the last case, upscaler selects upscaling amount by amount of tasks mapped when throughput hogs happen.
//!
//! **example scenario:** Let's say we have 10_000 tasks where every one of them is blocking for 1 second. Scheduler will map plenty of tasks but will get rejected.
//! This makes estimation calculation nearly 0 for both entering and exiting parts. When this happens and we still see tasks mapped from scheduler.
//! We start to slowly increase threads by amount of frequency linearly. High increase of this value either make us hit to the thread threshold on
//! some OS or make congestion on the other thread utilizations of the program, because of context switch.
//!
//! Throughput hogs determined by a combination of job in / job out frequency and current scheduler task assignment frequency.
//! Threshold of EMA difference is eluded by machine epsilon for floating point arithmetic errors.

use crate::worker::Sleeper;
use crate::{load_balancer, placement};
use core::fmt;
use crossbeam_channel::bounded;
use crossbeam_deque::{Injector, Stealer};
use crossbeam_queue::ArrayQueue;
use fmt::{Debug, Formatter};
use lazy_static::lazy_static;
use lightproc::lightproc::LightProc;
use placement::CoreId;
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    thread,
};
use tracing::{debug, trace};

/// The default thread park timeout before checking for new tasks.
const THREAD_PARK_TIMEOUT: Duration = Duration::from_millis(1);

/// Frequency histogram's sliding window size.
/// Defines how many frequencies will be considered for adaptation.
const FREQUENCY_QUEUE_SIZE: usize = 10;

/// If low watermark isn't configured this is the default scaler value.
/// This value is used for the heuristics of the scaler
const DEFAULT_LOW_WATERMARK: u64 = 2;

/// Pool scaler interval time (milliseconds).
/// This is the actual interval which makes adaptation calculation.
const SCALER_POLL_INTERVAL: u64 = 90;

/// Exponential moving average smoothing coefficient for limited window.
/// Smoothing factor is estimated with: 2 / (N + 1) where N is sample size.
const EMA_COEFFICIENT: f64 = 2_f64 / (FREQUENCY_QUEUE_SIZE as f64 + 1_f64);

lazy_static! {
    static ref ROUND_ROBIN_PIN: Mutex<CoreId> = Mutex::new(CoreId { id: 0 });
}

/// The `DynamicRunner` is piloted by `DynamicPoolManager`.
/// Upon request it needs to be able to provide runner routines for:
/// * Static threads.
/// * Dynamic threads.
/// * Standalone threads.
///
/// Your implementation of `DynamicRunner`
/// will allow you to define what tasks must be accomplished.
///
/// Run static threads:
///
/// run_static should never return, and park for park_timeout instead.
///
/// Run dynamic threads:
/// run_dynamic should never return, and call `parker()` when it has no more tasks to process.
/// It will be unparked automatically by the `DynamicPoolManager` if needs be.
///
/// Run standalone threads:
/// run_standalone should return once it has no more tasks to process.
/// The `DynamicPoolManager` will spawn other standalone threads if needs be.
pub trait DynamicRunner {
    fn setup(task_queue: Arc<Injector<LightProc>>) -> Sleeper<LightProc>;

    fn run_static<'b>(
        fences: impl Iterator<Item = &'b Stealer<LightProc>>,
        park_timeout: Duration,
    ) -> !;
    fn run_dynamic<'b>(fences: impl Iterator<Item = &'b Stealer<LightProc>>) -> !;
    fn run_standalone<'b>(fences: impl Iterator<Item = &'b Stealer<LightProc>>);
}

/// The `ThreadManager` is creates and destroys worker threads depending on demand according to
/// EMA rules.
///
/// It needs to be passed a structure that implements `DynamicRunner`,
/// That will be responsible for actually spawning threads.
///
/// The `DynamicPoolManager` keeps track of the number
/// of required number of threads to process load correctly.
/// and depending on the current state it will case it will:
/// - Spawn a lot of threads (we're predicting a load spike, and we need to prepare for it)
/// - Spawn few threads (there's a constant load, and throughput is low because the current resources are busy)
/// - Do nothing (the load is shrinking, threads will automatically stop once they're done).
///
/// Kinds of threads:
///
/// ## Static threads:
/// Defined in the constructor, they will always be available. They park for `THREAD_PARK_TIMEOUT` on idle.
///
/// ## Dynamic threads:
/// Created during `DynamicPoolManager` initialization, they will park on idle.
/// The `DynamicPoolManager` grows the number of Dynamic threads
/// so the total number of Static threads + Dynamic threads
/// is the number of available cores on the machine. (`num_cpus::get()`)
///
/// ## Standalone threads:
/// They are created when there aren't enough static and dynamic threads to process the expected load.
/// They will be destroyed on idle.
///
/// ## Spawn order:
/// In order to handle a growing load, the pool manager will ask to:
/// - Use Static threads
/// - Unpark Dynamic threads
/// - Spawn Standalone threads
///
/// The pool manager is not responsible for the tasks to be performed by the threads, it's handled by the `DynamicRunner`
///
/// If you use tracing, you can have a look at the trace! logs generated by the structure.
///
pub struct ThreadManager<Runner> {
    static_threads: usize,
    dynamic_threads: usize,
    parked_threads: ArrayQueue<Sleeper<LightProc>>,

    task_queue: Arc<Injector<LightProc>>,
    fences: Arc<RwLock<Vec<Stealer<LightProc>>>>,

    runner: Runner,
    last_frequency: AtomicU64,
}

impl<Runner: Debug> Debug for ThreadManager<Runner> {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        struct ThreadCount<'a>(&'a usize, &'a usize, &'a usize);
        impl<'a> Debug for ThreadCount<'a> {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("")
                    .field("static", self.0)
                    .field("dynamic", self.1)
                    .field("parked", self.2)
                    .finish()
            }
        }

        fmt.debug_struct("DynamicPoolManager")
            .field(
                "thread pool",
                &ThreadCount(
                    &self.static_threads,
                    &self.dynamic_threads,
                    &self.parked_threads.len(),
                ),
            )
            .field("runner", &self.runner)
            .field("last_frequency", &self.last_frequency)
            .finish()
    }
}

impl<Runner: DynamicRunner + Sync + Send> ThreadManager<Runner> {
    pub fn new(
        static_threads: usize,
        runner: Runner,
        task_queue: Arc<Injector<LightProc>>,
    ) -> Self {
        let dynamic_threads = 1.max(num_cpus::get().checked_sub(static_threads).unwrap_or(0));
        let parked_threads = ArrayQueue::new(1.max(static_threads + dynamic_threads));
        let fences = Arc::new(RwLock::new(Vec::new()));

        Self {
            static_threads,
            dynamic_threads,
            parked_threads,

            task_queue,
            fences,

            runner,
            last_frequency: AtomicU64::new(0),
        }
    }

    pub fn increment_frequency(&self) {
        self.last_frequency.fetch_add(1, Ordering::Acquire);
    }

    /// Initialize the dynamic pool
    /// That will be scaled
    pub(crate) fn initialize(&'static self) {
        let (tx, rx) = bounded(self.static_threads + self.dynamic_threads);

        let fencelock = &self.fences;
        let _guard = fencelock.write().unwrap();

        let mut i = 0;

        // Static thread manager that will always be available
        debug!("spooling up {} static worker threads", self.static_threads);
        (0..self.static_threads).for_each(|_| {
            let tx = tx.clone();
            let fencelock = fencelock.clone();
            let task_queue = self.task_queue.clone();
            thread::Builder::new()
                .name(format!("rt({}) [static]", i))
                .spawn(move || {
                    Self::affinity_pinner();

                    let sleeper = Runner::setup(task_queue);
                    tx.send(sleeper).expect("Failed to push to parked_threads");
                    drop(tx);

                    let fencelock = fencelock.clone();
                    let fences = fencelock.read().unwrap();

                    Runner::run_static(fences.iter(), THREAD_PARK_TIMEOUT);
                })
                .expect("failed to spawn static worker thread");
            i += 1;
        });

        // Dynamic thread manager that will allow us to unpark threads when needed
        debug!(
            "spooling up {} dynamic worker threads",
            self.dynamic_threads
        );
        (0..self.dynamic_threads).for_each(|_| {
            let tx = tx.clone();
            let fencelock = fencelock.clone();
            let task_queue = self.task_queue.clone();
            thread::Builder::new()
                .name(format!("rt({}) [dyn]", i))
                .spawn(move || {
                    Self::affinity_pinner();

                    let sleeper = Runner::setup(task_queue);
                    tx.send(sleeper).expect("Failed to push to parked_threads");
                    drop(tx);

                    let fences = fencelock.read().unwrap();

                    Runner::run_dynamic(fences.iter());
                })
                .expect("failed to spawn dynamic worker thread");
            i += 1;
        });

        drop(tx);

        let span = tracing::span!(tracing::Level::INFO, "sleepers").entered();
        tracing::info!("Spawned {} threads", i);
        for _ in 0..i {
            let sleeper = rx.recv().unwrap();
            tracing::info!("{:?}", &sleeper);
            self.parked_threads.push(sleeper).unwrap();
        }
        span.exit();

        // Pool manager to check frequency of task rates
        // and take action by scaling the pool accordingly.
        thread::Builder::new()
            .name("pool manager".to_string())
            .spawn(move || {
                let poll_interval = Duration::from_millis(SCALER_POLL_INTERVAL);
                debug!("setting up the pool manager");
                loop {
                    self.scale_pool();
                    thread::park_timeout(poll_interval);
                }
            })
            .expect("failed to spawn pool manager thread");
    }

    /// Provision threads takes a number of threads that need to be made available.
    /// It will try to unpark threads from the dynamic pool, and spawn more threads if needs be.
    pub fn provision_threads(
        &'static self,
        n: usize,
        fencelock: &Arc<RwLock<Vec<Stealer<LightProc>>>>,
    ) {
        let rem = self.unpark_thread(n);
        if rem != 0 {
            debug!("no more threads to unpark, spawning {} new threads", rem);
            //self.spawn_threads(rem, fencelock);
        }
    }

    fn spawn_threads(&'static self, n: usize, fencelock: &Arc<RwLock<Vec<Stealer<LightProc>>>>) {
        (0..n).for_each(|_| {
            let fencelock = fencelock.clone();
            let task_queue = self.task_queue.clone();
            thread::Builder::new()
                .name("standalone worker".to_string())
                .spawn(move || {
                    Self::affinity_pinner();
                    let _ = Runner::setup(task_queue);
                    let fences = fencelock.read().unwrap();
                    Runner::run_standalone(fences.iter());
                })
                .unwrap();
        })
    }

    /// Tries to unpark the given number of threads.
    /// Returns `num - (number of threads unparked)`
    fn unpark_thread(&self, num: usize) -> usize {
        let len = self.parked_threads.len();
        debug!("parked_threads: len is {}", len);
        // Only check threads once
        for _ in 0..len {
            if let Some(thread) = self.parked_threads.pop() {
                thread.wakeup();
            }
        }

        num
    }

    /// Affinity pinner for blocking pool
    ///
    /// Pinning isn't going to be enabled for single core systems.
    #[inline]
    fn affinity_pinner() {
        if 1 != *load_balancer::core_count() {
            let mut core = ROUND_ROBIN_PIN.lock().unwrap();
            placement::set_for_current(*core);
            core.id = (core.id + 1) % *load_balancer::core_count();
        }
    }

    /// Exponentially Weighted Moving Average calculation
    ///
    /// This allows us to find the EMA value.
    /// This value represents the trend of tasks mapped onto the thread pool.
    /// Calculation is following:
    /// ```text
    /// +--------+-----------------+----------------------------------+
    /// | Symbol |   Identifier    |           Explanation            |
    /// +--------+-----------------+----------------------------------+
    /// | α      | EMA_COEFFICIENT | smoothing factor between 0 and 1 |
    /// | Yt     | freq            | frequency sample at time t       |
    /// | St     | acc             | EMA at time t                    |
    /// +--------+-----------------+----------------------------------+
    /// ```
    /// Under these definitions formula is following:
    /// ```text
    /// EMA = α * [ Yt + (1 - α)*Yt-1 + ((1 - α)^2)*Yt-2 + ((1 - α)^3)*Yt-3 ... ] + St
    /// ```
    /// # Arguments
    ///
    /// * `freq_queue` - Sliding window of frequency samples
    #[inline]
    fn calculate_ema(freq_queue: &VecDeque<u64>) -> f64 {
        freq_queue.iter().enumerate().fold(0_f64, |acc, (i, freq)| {
            acc + ((*freq as f64) * ((1_f64 - EMA_COEFFICIENT).powf(i as f64) as f64))
        }) * EMA_COEFFICIENT as f64
    }

    /// Adaptive pool scaling function
    ///
    /// This allows to spawn new threads to make room for incoming task pressure.
    /// Works in the background detached from the pool system and scales up the pool based
    /// on the request rate.
    ///
    /// It uses frequency based calculation to define work. Utilizing average processing rate.
    fn scale_pool(&'static self) {}
}
