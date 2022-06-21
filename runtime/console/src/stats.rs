use crate::aggregate::Id;
use crate::attribute;
use crossbeam_utils::atomic::AtomicCell;
use hdrhistogram::serialization::{Serializer, V2Serializer};
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

/// Anchors an `Instant` with a `SystemTime` timestamp to allow converting
/// monotonic `Instant`s into timestamps that can be sent over the wire.
#[derive(Debug, Clone)]
pub(crate) struct TimeAnchor {
    mono: Instant,
    sys: SystemTime,
}

impl TimeAnchor {
    pub(crate) fn new() -> Self {
        Self {
            mono: Instant::now(),
            sys: SystemTime::now(),
        }
    }

    pub(crate) fn to_system_time(&self, t: Instant) -> SystemTime {
        let dur = t
            .checked_duration_since(self.mono)
            .unwrap_or_else(|| Duration::from_secs(0));
        self.sys + dur
    }

    pub(crate) fn to_timestamp(&self, t: Instant) -> prost_types::Timestamp {
        self.to_system_time(t).into()
    }
}

#[derive(Debug, Default)]
struct PollStats<H> {
    /// The number of polls in progress
    current_polls: AtomicUsize,
    /// The total number of polls
    polls: AtomicUsize,
    timestamps: Mutex<PollTimestamps<H>>,
}

/// Stats associated with a task.
#[derive(Debug)]
pub(crate) struct TaskStats {
    is_dirty: AtomicBool,
    is_dropped: AtomicBool,
    // task stats
    pub(crate) created_at: Instant,
    timestamps: Mutex<TaskTimestamps>,

    // waker stats
    wakes: AtomicUsize,
    waker_clones: AtomicUsize,
    waker_drops: AtomicUsize,
    self_wakes: AtomicUsize,

    /// Poll durations and other stats.
    poll_stats: PollStats<Histogram>,
}

impl TaskStats {
    pub(crate) fn poll_duration_histogram(
        &self,
    ) -> console_api::tasks::task_details::PollTimesHistogram {
        let hist = self
            .poll_stats
            .timestamps
            .lock()
            .unwrap()
            .histogram
            .to_proto();
        console_api::tasks::task_details::PollTimesHistogram::Histogram(hist)
    }
}

/// Stats associated with an async operation.
///
/// This shares all of the same fields as [`ResourceStats]`, with the addition
/// of [`PollStats`] tracking when the async operation is polled, and the task
/// ID of the last task to poll the async op.
#[derive(Debug)]
pub(crate) struct AsyncOpStats {
    /// The task ID of the last task to poll this async op.
    ///
    /// This is set every time the async op is polled, in case a future is
    /// passed between tasks.
    task_id: AtomicCell<u64>,

    /// Fields shared with `ResourceStats`.
    pub(crate) stats: ResourceStats,

    /// Poll durations and other stats.
    poll_stats: PollStats<()>,
}

/// Stats associated with a resource.
#[derive(Debug)]
pub(crate) struct ResourceStats {
    is_dirty: AtomicBool,
    is_dropped: AtomicBool,
    created_at: Instant,
    dropped_at: Mutex<Option<Instant>>,
    attributes: Mutex<attribute::Attributes>,
    pub(crate) inherit_child_attributes: bool,
    pub(crate) parent_id: Option<Id>,
}

#[derive(Debug, Default)]
struct TaskTimestamps {
    dropped_at: Option<Instant>,
    last_wake: Option<Instant>,
}

#[derive(Debug, Default)]
struct PollTimestamps<H> {
    first_poll: Option<Instant>,
    last_poll_started: Option<Instant>,
    last_poll_ended: Option<Instant>,
    busy_time: Duration,
    histogram: H,
}

#[derive(Debug)]
struct Histogram {
    histogram: hdrhistogram::Histogram<u64>,
    max: u64,
    outliers: u64,
    max_outlier: Option<u64>,
}

impl Histogram {
    fn new(max: u64) -> Self {
        // significant figures should be in the [0-5] range and memory usage
        // grows exponentially with higher a sigfig
        let histogram = hdrhistogram::Histogram::new_with_max(max, 2).unwrap();
        Self {
            histogram,
            max,
            max_outlier: None,
            outliers: 0,
        }
    }

    fn to_proto(&self) -> console_api::tasks::DurationHistogram {
        let mut serializer = V2Serializer::new();
        let mut raw_histogram = Vec::new();
        serializer
            .serialize(&self.histogram, &mut raw_histogram)
            .expect("histogram failed to serialize");
        console_api::tasks::DurationHistogram {
            raw_histogram,
            max_value: self.max,
            high_outliers: self.outliers,
            highest_outlier: self.max_outlier,
        }
    }
}
