use crate::id_map::{IdMap, ToProto};
use crate::server::{Watch, WatchRequest};
use crate::stats::{TimeAnchor, Unsent};
use crate::{server, stats};
use crate::{Event, Shared};
use console_api::{async_ops, instrument, resources, tasks};
use crossbeam_channel::{Receiver, TryRecvError};
use futures_util::{FutureExt, StreamExt};
use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::span;
use tracing_core::Metadata;

#[derive(Debug)]
struct Resource {
    id: span::Id,
    is_dirty: AtomicBool,
    parent_id: Option<span::Id>,
    metadata: &'static Metadata<'static>,
    concrete_type: String,
    kind: resources::resource::Kind,
    location: Option<console_api::Location>,
    is_internal: bool,
}

/// Represents static data for tasks
#[derive(Debug)]
struct Task {
    id: span::Id,
    is_dirty: AtomicBool,
    metadata: &'static Metadata<'static>,
    fields: Vec<console_api::Field>,
    location: Option<console_api::Location>,
}

#[derive(Debug)]
struct AsyncOp {
    id: span::Id,
    is_dirty: AtomicBool,
    parent_id: Option<span::Id>,
    resource_id: span::Id,
    metadata: &'static Metadata<'static>,
    source: String,
}

impl ToProto for Task {
    type Output = tasks::Task;

    fn to_proto(&self, _: &stats::TimeAnchor) -> Self::Output {
        tasks::Task {
            id: Some(self.id.clone().into()),
            // TODO: more kinds of tasks...
            kind: tasks::task::Kind::Spawn as i32,
            metadata: Some(self.metadata.into()),
            parents: Vec::new(), // TODO: implement parents nicely
            fields: self.fields.clone(),
            location: self.location.clone(),
        }
    }
}

impl Unsent for Task {
    fn take_unsent(&self) -> bool {
        self.is_dirty.swap(false, Ordering::AcqRel)
    }

    fn is_unsent(&self) -> bool {
        self.is_dirty.load(Ordering::Acquire)
    }
}

impl ToProto for Resource {
    type Output = resources::Resource;

    fn to_proto(&self, _: &stats::TimeAnchor) -> Self::Output {
        resources::Resource {
            id: Some(self.id.clone().into()),
            parent_resource_id: self.parent_id.clone().map(Into::into),
            kind: Some(self.kind.clone()),
            metadata: Some(self.metadata.into()),
            concrete_type: self.concrete_type.clone(),
            location: self.location.clone(),
            is_internal: self.is_internal,
        }
    }
}

impl Unsent for Resource {
    fn take_unsent(&self) -> bool {
        self.is_dirty.swap(false, Ordering::AcqRel)
    }

    fn is_unsent(&self) -> bool {
        self.is_dirty.load(Ordering::Acquire)
    }
}

impl ToProto for AsyncOp {
    type Output = async_ops::AsyncOp;

    fn to_proto(&self, _: &stats::TimeAnchor) -> Self::Output {
        async_ops::AsyncOp {
            id: Some(self.id.clone().into()),
            metadata: Some(self.metadata.into()),
            resource_id: Some(self.resource_id.clone().into()),
            source: self.source.clone(),
            parent_async_op_id: self.parent_id.clone().map(Into::into),
        }
    }
}

impl Unsent for AsyncOp {
    fn take_unsent(&self) -> bool {
        self.is_dirty.swap(false, Ordering::AcqRel)
    }

    fn is_unsent(&self) -> bool {
        self.is_dirty.load(Ordering::Acquire)
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub(crate) enum Include {
    All,
    UpdatedOnly,
}

#[derive(Debug)]
pub struct Aggregator {
    shared: Arc<Shared>,
    events: Receiver<Event>,
    rpcs: async_channel::Receiver<server::Command>,
    watchers: Vec<Watch<instrument::Update>>,
    details_watchers: HashMap<span::Id, Vec<Watch<tasks::TaskDetails>>>,
    all_metadata: Vec<console_api::register_metadata::NewMetadata>,
    new_metadata: Vec<console_api::register_metadata::NewMetadata>,
    running: bool,
    publish_interval: Duration,
    base_time: TimeAnchor,
    tasks: IdMap<Task>,
    task_stats: IdMap<Arc<stats::TaskStats>>,
    resources: IdMap<Resource>,
    resource_stats: IdMap<Arc<stats::ResourceStats>>,
    async_ops: IdMap<AsyncOp>,
    async_op_stats: IdMap<Arc<stats::AsyncOpStats>>,
    poll_ops: Vec<console_api::resources::PollOp>,
}

impl Aggregator {
    pub(crate) fn new(
        shared: Arc<Shared>,
        events: Receiver<Event>,
        rpcs: async_channel::Receiver<server::Command>,
    ) -> Self {
        Self {
            shared,
            events,
            rpcs,
            watchers: Vec::new(),
            details_watchers: HashMap::default(),
            running: true,
            publish_interval: Duration::from_secs(1),
            all_metadata: Vec::new(),
            new_metadata: Vec::new(),
            base_time: TimeAnchor::new(),
            tasks: IdMap::default(),
            task_stats: IdMap::default(),
            resources: IdMap::default(),
            resource_stats: IdMap::default(),
            async_ops: IdMap::default(),
            async_op_stats: IdMap::default(),
            poll_ops: Vec::new(),
        }
    }

    fn add_instrument_subscription(&mut self, subscription: Watch<instrument::Update>) {
        tracing::debug!("new instrument subscription");

        let task_update = Some(self.task_update(Include::All));
        let resource_update = Some(self.resource_update(Include::All));
        let async_op_update = Some(self.async_op_update(Include::All));
        let now = Instant::now();

        let update = &instrument::Update {
            task_update,
            resource_update,
            async_op_update,
            now: Some(self.base_time.to_timestamp(now)),
            new_metadata: Some(console_api::RegisterMetadata {
                metadata: (self.all_metadata).clone(),
            }),
        };

        // Send the initial state --- if this fails, the subscription is already dead
        if subscription.update(update) {
            self.watchers.push(subscription)
        }
    }

    /// Add the task details subscription to the watchers after sending the first update,
    /// if the task is found.
    fn add_task_detail_subscription(
        &mut self,
        watch_request: WatchRequest<console_api::tasks::TaskDetails>,
    ) {
        let WatchRequest {
            id,
            mut stream_sender,
            buffer,
        } = watch_request;
        tracing::debug!(id = ?id, "new task details subscription");
        if let Some(stats) = self.task_stats.get(&id) {
            let (tx, rx) = async_channel::bounded(buffer);
            let subscription = Watch(tx);
            let now = Some(self.base_time.to_timestamp(Instant::now()));
            // Send back the stream receiver.
            // Then send the initial state --- if this fails, the subscription is already dead.
            if stream_sender.send(rx).is_ok()
                && subscription.update(&console_api::tasks::TaskDetails {
                    task_id: Some(id.clone().into()),
                    now,
                    poll_times_histogram: Some(stats.poll_duration_histogram()),
                })
            {
                self.details_watchers
                    .entry(id.clone())
                    .or_insert_with(Vec::new)
                    .push(subscription);
            }
        }
        // If the task is not found, drop `stream_sender` which will result in a not found error
    }

    fn task_update(&mut self, include: Include) -> tasks::TaskUpdate {
        tasks::TaskUpdate {
            new_tasks: self.tasks.as_proto_list(include, &self.base_time),
            stats_update: self.task_stats.as_proto(include, &self.base_time),
            dropped_events: self.shared.dropped_tasks.swap(0, Ordering::AcqRel) as u64,
        }
    }

    fn resource_update(&mut self, include: Include) -> resources::ResourceUpdate {
        let new_poll_ops = match include {
            Include::All => self.poll_ops.clone(),
            Include::UpdatedOnly => std::mem::take(&mut self.poll_ops),
        };
        resources::ResourceUpdate {
            new_resources: self.resources.as_proto_list(include, &self.base_time),
            stats_update: self.resource_stats.as_proto(include, &self.base_time),
            new_poll_ops,
            dropped_events: self.shared.dropped_resources.swap(0, Ordering::AcqRel) as u64,
        }
    }

    fn async_op_update(&mut self, include: Include) -> async_ops::AsyncOpUpdate {
        async_ops::AsyncOpUpdate {
            new_async_ops: self.async_ops.as_proto_list(include, &self.base_time),
            stats_update: self.async_op_stats.as_proto(include, &self.base_time),
            dropped_events: self.shared.dropped_async_ops.swap(0, Ordering::AcqRel) as u64,
        }
    }

    pub async fn run(mut self) {
        let mut timer = StreamExt::fuse(async_io::Timer::interval(self.publish_interval));
        loop {
            let mut recv = self.rpcs.recv().fuse();
            let should_send: bool = futures_util::select! {
                _ = timer.next() => self.running,
                cmd = recv => {
                    match cmd {
                        Ok(server::Command::Instrument(subscription)) => {
                            self.add_instrument_subscription(subscription);
                        }
                        Ok(server::Command::WatchTaskDetail(request)) => {
                        }
                        Ok(server::Command::Pause) => {
                            self.running = false;
                        }
                        Ok(server::Command::Resume) => {
                            self.running = true;
                        }
                        Err(_) => {
                            tracing::debug!("rpc channel closed, exiting");
                            return
                        }
                    }
                    false
                },
            };

            // drain and aggregate buffered events.
            //
            // Note: we *don't* want to actually await the call to `recv` --- we
            // don't want the aggregator task to be woken on every event,
            // because it will then be woken when its own `poll` calls are
            // exited. that would result in a busy-loop. instead, we only want
            // to be woken when the flush interval has elapsed, or when the
            // channel is almost full.
            while let Ok(event) = self.events.try_recv() {
                self.update_state(event);
            }
            if let Err(TryRecvError::Disconnected) = self.events.try_recv() {
                tracing::debug!("event channel closed; terminating");
                return;
            }

            // flush data to clients, if there are any currently subscribed
            // watchers and we should send a new update.
            if !self.watchers.is_empty() && should_send {
                self.publish();
            }
        }
    }

    fn publish(&mut self) {
        let new_metadata = if !self.new_metadata.is_empty() {
            Some(console_api::RegisterMetadata {
                metadata: std::mem::take(&mut self.new_metadata),
            })
        } else {
            None
        };
        let task_update = Some(self.task_update(Include::UpdatedOnly));
        let resource_update = Some(self.resource_update(Include::UpdatedOnly));
        let async_op_update = Some(self.async_op_update(Include::UpdatedOnly));

        let update = instrument::Update {
            now: Some(self.base_time.to_timestamp(Instant::now())),
            new_metadata,
            task_update,
            resource_update,
            async_op_update,
        };

        self.watchers
            .retain(|watch: &Watch<instrument::Update>| watch.update(&update));

        let stats = &self.task_stats;
        // Assuming there are much fewer task details subscribers than there are
        // stats updates, iterate over `details_watchers` and compact the map.
        self.details_watchers.retain(|id, watchers| {
            if let Some(task_stats) = stats.get(id) {
                let details = tasks::TaskDetails {
                    task_id: Some(id.clone().into()),
                    now: Some(self.base_time.to_timestamp(Instant::now())),
                    poll_times_histogram: Some(task_stats.poll_duration_histogram()),
                };
                watchers.retain(|watch| watch.update(&details));
                !watchers.is_empty()
            } else {
                false
            }
        });
    }

    /// Update the current state with data from a single event.
    fn update_state(&mut self, event: Event) {
        // do state update
        match event {
            Event::Metadata(meta) => {
                self.all_metadata.push(meta.into());
                self.new_metadata.push(meta.into());
            }

            Event::Spawn {
                id,
                metadata,
                stats,
                fields,
                location,
            } => {
                self.tasks.insert(
                    id.clone(),
                    Task {
                        id: id.clone(),
                        is_dirty: AtomicBool::new(true),
                        metadata,
                        fields,
                        location,
                        // TODO: parents
                    },
                );

                self.task_stats.insert(id, stats);
            }

            Event::Resource {
                id,
                parent_id,
                metadata,
                kind,
                concrete_type,
                location,
                is_internal,
                stats,
            } => {
                self.resources.insert(
                    id.clone(),
                    Resource {
                        id: id.clone(),
                        is_dirty: AtomicBool::new(true),
                        parent_id,
                        kind,
                        metadata,
                        concrete_type,
                        location,
                        is_internal,
                    },
                );

                self.resource_stats.insert(id, stats);
            }

            Event::PollOp {
                metadata,
                resource_id,
                op_name,
                async_op_id,
                task_id,
                is_ready,
            } => {
                let poll_op = resources::PollOp {
                    metadata: Some(metadata.into()),
                    resource_id: Some(resource_id.into()),
                    name: op_name,
                    task_id: Some(task_id.into()),
                    async_op_id: Some(async_op_id.into()),
                    is_ready,
                };

                self.poll_ops.push(poll_op);
            }

            Event::AsyncResourceOp {
                id,
                source,
                resource_id,
                metadata,
                parent_id,
                stats,
            } => {
                self.async_ops.insert(
                    id.clone(),
                    AsyncOp {
                        id: id.clone(),
                        is_dirty: AtomicBool::new(true),
                        resource_id,
                        metadata,
                        source,
                        parent_id,
                    },
                );

                self.async_op_stats.insert(id, stats);
            }
        }
    }
}
