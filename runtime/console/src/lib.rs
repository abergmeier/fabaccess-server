use crossbeam_channel::{Sender, TrySendError};
use std::borrow::Borrow;
use std::cell::RefCell;
use std::net::IpAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use thread_local::ThreadLocal;
use tracing::span;
use tracing_core::span::Attributes;
use tracing_core::{Interest, Metadata, Subscriber};
use tracing_subscriber::layer::{Context, Filter};
use tracing_subscriber::registry::{LookupSpan, SpanRef};
use tracing_subscriber::Layer;

mod aggregate;
mod attribute;
mod callsites;
mod event;
mod id_map;
mod server;
mod stack;
mod stats;
mod visitors;

use crate::aggregate::Aggregator;
use crate::callsites::Callsites;
use crate::visitors::{
    AsyncOpVisitor, PollOpVisitor, ResourceVisitor, ResourceVisitorResult, StateUpdateVisitor,
    TaskVisitor, WakerVisitor,
};
use event::Event;
pub use server::Server;
use stack::SpanStack;

#[derive(Debug)]
pub struct ConsoleLayer {
    current_spans: ThreadLocal<RefCell<SpanStack>>,

    tx: Sender<Event>,
    shared: Arc<Shared>,

    spawn_callsites: Callsites<8>,
    waker_callsites: Callsites<8>,
    resource_callsites: Callsites<8>,

    /// Set of callsites for spans representing async operations on resources
    ///
    /// TODO: Take some time to determine more reasonable numbers
    async_op_callsites: Callsites<32>,

    /// Set of callsites for spans representing async op poll operations
    ///
    /// TODO: Take some time to determine more reasonable numbers
    async_op_poll_callsites: Callsites<32>,

    /// Set of callsites for events representing poll operation invocations on resources
    ///
    /// TODO: Take some time to determine more reasonable numbers
    poll_op_callsites: Callsites<32>,

    /// Set of callsites for events representing state attribute state updates on resources
    ///
    /// TODO: Take some time to determine more reasonable numbers
    resource_state_update_callsites: Callsites<32>,

    /// Set of callsites for events representing state attribute state updates on async resource ops
    ///
    /// TODO: Take some time to determine more reasonable numbers
    async_op_state_update_callsites: Callsites<32>,

    max_poll_duration_nanos: u64,
}

#[derive(Debug)]
pub struct Builder {
    /// Network Address the console server will listen on
    server_addr: IpAddr,
    /// Network Port the console server will listen on
    server_port: u16,

    /// Number of events that can be buffered before events are dropped.
    ///
    /// A smaller number will reduce the memory footprint but may lead to more events being dropped
    /// during activity bursts.
    event_buffer_capacity: usize,

    client_buffer_capacity: usize,

    poll_duration_max: Duration,
}
impl Builder {
    pub fn build(self) -> (ConsoleLayer, Server) {
        ConsoleLayer::build(self)
    }
}
impl Default for Builder {
    fn default() -> Self {
        Self {
            // Listen on `::1` (aka localhost) by default
            server_addr: Server::DEFAULT_ADDR,
            server_port: Server::DEFAULT_PORT,
            event_buffer_capacity: ConsoleLayer::DEFAULT_EVENT_BUFFER_CAPACITY,
            client_buffer_capacity: 1024,
            poll_duration_max: ConsoleLayer::DEFAULT_POLL_DURATION_MAX,
        }
    }
}

#[derive(Debug, Default)]
struct Shared {
    dropped_tasks: AtomicUsize,
    dropped_resources: AtomicUsize,
    dropped_async_ops: AtomicUsize,
}

impl ConsoleLayer {
    pub fn new() -> (Self, Server) {
        Self::builder().build()
    }
    pub fn builder() -> Builder {
        Builder::default()
    }
    fn build(config: Builder) -> (Self, Server) {
        tracing::debug!(
            ?config.server_addr,
            config.event_buffer_capacity,
            "configured console subscriber"
        );

        let (tx, events) = crossbeam_channel::bounded(config.event_buffer_capacity);
        let shared = Arc::new(Shared::default());
        let (subscribe, rpcs) = async_channel::bounded(config.client_buffer_capacity);
        let aggregator = Aggregator::new(shared.clone(), events, rpcs);
        let server = Server::new(aggregator, config.client_buffer_capacity, subscribe);
        let layer = Self {
            current_spans: ThreadLocal::new(),
            tx,
            shared,
            spawn_callsites: Callsites::default(),
            waker_callsites: Callsites::default(),
            resource_callsites: Callsites::default(),
            async_op_callsites: Callsites::default(),
            async_op_poll_callsites: Callsites::default(),
            poll_op_callsites: Callsites::default(),
            resource_state_update_callsites: Callsites::default(),
            async_op_state_update_callsites: Callsites::default(),
            max_poll_duration_nanos: config.poll_duration_max.as_nanos() as u64,
        };

        (layer, server)
    }
}

impl ConsoleLayer {
    const DEFAULT_EVENT_BUFFER_CAPACITY: usize = 1024;
    const DEFAULT_CLIENT_BUFFER_CAPACITY: usize = 1024;

    /// The default maximum value for task poll duration histograms.
    ///
    /// Any poll duration exceeding this will be clamped to this value. By
    /// default, the maximum poll duration is one second.
    ///
    /// See also [`Builder::poll_duration_histogram_max`].
    pub const DEFAULT_POLL_DURATION_MAX: Duration = Duration::from_secs(1);

    fn is_spawn(&self, metadata: &Metadata<'static>) -> bool {
        self.spawn_callsites.contains(metadata)
    }

    fn is_waker(&self, metadata: &Metadata<'static>) -> bool {
        self.waker_callsites.contains(metadata)
    }

    fn is_resource(&self, meta: &'static Metadata<'static>) -> bool {
        self.resource_callsites.contains(meta)
    }

    fn is_async_op(&self, meta: &'static Metadata<'static>) -> bool {
        self.async_op_callsites.contains(meta)
    }

    fn is_id_spawned<S>(&self, id: &span::Id, cx: &Context<'_, S>) -> bool
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        cx.span(id)
            .map(|span| self.is_spawn(span.metadata()))
            .unwrap_or(false)
    }

    fn is_id_resource<S>(&self, id: &span::Id, cx: &Context<'_, S>) -> bool
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        cx.span(id)
            .map(|span| self.is_resource(span.metadata()))
            .unwrap_or(false)
    }

    fn is_id_async_op<S>(&self, id: &span::Id, cx: &Context<'_, S>) -> bool
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        cx.span(id)
            .map(|span| self.is_async_op(span.metadata()))
            .unwrap_or(false)
    }

    fn first_entered<P>(&self, stack: &SpanStack, p: P) -> Option<span::Id>
    where
        P: Fn(&span::Id) -> bool,
    {
        stack
            .stack()
            .iter()
            .rev()
            .find(|id| p(id.id()))
            .map(|id| id.id())
            .cloned()
    }

    fn send_stats<S>(
        &self,
        dropped: &AtomicUsize,
        mkEvent: impl FnOnce() -> (Event, S),
    ) -> Option<S> {
        if self.tx.is_full() {
            dropped.fetch_add(1, Ordering::Release);
            return None;
        }

        let (event, stats) = mkEvent();
        match self.tx.try_send(event) {
            Ok(()) => Some(stats),
            Err(TrySendError::Full(_)) => {
                dropped.fetch_add(1, Ordering::Release);
                None
            }
            Err(TrySendError::Disconnected(_)) => None,
        }
    }

    fn send_metadata(&self, dropped: &AtomicUsize, event: Event) -> bool {
        self.send_stats(dropped, || (event, ())).is_some()
    }
}

impl<S> Layer<S> for ConsoleLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn register_callsite(&self, metadata: &'static Metadata<'static>) -> Interest {
        let dropped = match (metadata.name(), metadata.target()) {
            (_, TaskVisitor::SPAWN_TARGET) | (TaskVisitor::SPAWN_NAME, _) => {
                self.spawn_callsites.insert(metadata);
                &self.shared.dropped_tasks
            }
            (WakerVisitor::WAKE_TARGET, _) => {
                self.waker_callsites.insert(metadata);
                &self.shared.dropped_tasks
            }
            (ResourceVisitor::RES_SPAN_NAME, _) => {
                self.resource_callsites.insert(metadata);
                &self.shared.dropped_resources
            }
            (AsyncOpVisitor::ASYNC_OP_SPAN_NAME, _) => {
                self.async_op_callsites.insert(metadata);
                &self.shared.dropped_async_ops
            }
            (AsyncOpVisitor::ASYNC_OP_POLL_NAME, _) => {
                self.async_op_poll_callsites.insert(metadata);
                &self.shared.dropped_async_ops
            }
            (_, PollOpVisitor::POLL_OP_EVENT_TARGET) => {
                self.poll_op_callsites.insert(metadata);
                &self.shared.dropped_async_ops
            }
            (_, StateUpdateVisitor::RE_STATE_UPDATE_EVENT_TARGET) => {
                self.resource_state_update_callsites.insert(metadata);
                &self.shared.dropped_resources
            }
            (_, StateUpdateVisitor::AO_STATE_UPDATE_EVENT_TARGET) => {
                self.async_op_state_update_callsites.insert(metadata);
                &self.shared.dropped_async_ops
            }
            (_, _) => &self.shared.dropped_tasks,
        };

        self.send_metadata(dropped, Event::Metadata(metadata));

        Interest::always()
    }

    fn on_new_span(&self, attrs: &Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        let metadata = attrs.metadata();
        if self.is_spawn(metadata) {
            let at = Instant::now();
            let mut task_visitor = TaskVisitor::new(metadata.into());
            attrs.record(&mut task_visitor);
            let (fields, location) = task_visitor.result();
            if let Some(stats) = self.send_stats(&self.shared.dropped_tasks, move || {
                let stats = Arc::new(stats::TaskStats::new(self.max_poll_duration_nanos, at));
                let event = Event::Spawn {
                    id: id.clone(),
                    stats: stats.clone(),
                    metadata,
                    fields,
                    location,
                };
                (event, stats)
            }) {
                ctx.span(id)
                    .expect("`on_new_span` called with nonexistent span. This is a tracing bug.");
            }
        } else if self.is_resource(metadata) {
            let at = Instant::now();
            let mut resource_visitor = ResourceVisitor::default();
            attrs.record(&mut resource_visitor);
            if let Some(result) = resource_visitor.result() {
                let ResourceVisitorResult {
                    concrete_type,
                    kind,
                    location,
                    is_internal,
                    inherit_child_attrs,
                } = result;
                let parent_id = self.current_spans.get().and_then(|stack| {
                    self.first_entered(&stack.borrow(), |id| self.is_id_resource(id, &ctx))
                });
                if let Some(stats) = self.send_stats(&self.shared.dropped_resources, move || {
                    let stats = Arc::new(stats::ResourceStats::new(
                        at,
                        inherit_child_attrs,
                        parent_id.clone(),
                    ));
                    let event = Event::Resource {
                        id: id.clone(),
                        parent_id,
                        metadata,
                        concrete_type,
                        kind,
                        location,
                        is_internal,
                        stats: stats.clone(),
                    };
                    (event, stats)
                }) {
                    ctx.span(id).expect("if `on_new_span` was called, the span must exist; this is a `tracing` bug!").extensions_mut().insert(stats);
                }
            }
        } else if self.is_async_op(metadata) {
            let at = Instant::now();
            let mut async_op_visitor = AsyncOpVisitor::default();
            attrs.record(&mut async_op_visitor);
            if let Some((source, inherit_child_attrs)) = async_op_visitor.result() {
                let resource_id = self.current_spans.get().and_then(|stack| {
                    self.first_entered(&stack.borrow(), |id| self.is_id_resource(id, &ctx))
                });

                let parent_id = self.current_spans.get().and_then(|stack| {
                    self.first_entered(&stack.borrow(), |id| self.is_id_async_op(id, &ctx))
                });

                if let Some(resource_id) = resource_id {
                    if let Some(stats) =
                        self.send_stats(&self.shared.dropped_async_ops, move || {
                            let stats = Arc::new(stats::AsyncOpStats::new(
                                at,
                                inherit_child_attrs,
                                parent_id.clone(),
                            ));
                            let event = Event::AsyncResourceOp {
                                id: id.clone(),
                                parent_id,
                                resource_id,
                                metadata,
                                source,
                                stats: stats.clone(),
                            };
                            (event, stats)
                        })
                    {
                        ctx.span(id).expect("if `on_new_span` was called, the span must exist; this is a `tracing` bug!").extensions_mut().insert(stats);
                    }
                }
            }
        }
    }

    fn on_event(&self, event: &tracing::Event<'_>, ctx: Context<'_, S>) {
        let metadata = event.metadata();
        if self.waker_callsites.contains(metadata) {
            let at = Instant::now();
            let mut visitor = WakerVisitor::default();
            event.record(&mut visitor);
            if let Some((id, mut op)) = visitor.result() {
                if let Some(span) = ctx.span(&id) {
                    let exts = span.extensions();
                    if let Some(stats) = exts.get::<Arc<stats::TaskStats>>() {
                        if op.is_wake() {
                            let self_wake = self
                                .current_spans
                                .get()
                                .map(|spans| spans.borrow().iter().any(|span| span == &id))
                                .unwrap_or(false);
                            op = op.self_wake(self_wake);
                        }

                        stats.record_wake_op(op, at);
                    }
                }
            }
        }
    }

    fn on_enter(&self, id: &span::Id, cx: Context<'_, S>) {
        fn update<S: Subscriber + for<'a> LookupSpan<'a>>(
            span: &SpanRef<S>,
            at: Option<Instant>,
        ) -> Option<Instant> {
            let exts = span.extensions();
            // if the span we are entering is a task or async op, record the
            // poll stats.
            if let Some(stats) = exts.get::<Arc<stats::TaskStats>>() {
                let at = at.unwrap_or_else(Instant::now);
                stats.start_poll(at);
                Some(at)
            } else if let Some(stats) = exts.get::<Arc<stats::AsyncOpStats>>() {
                let at = at.unwrap_or_else(Instant::now);
                stats.start_poll(at);
                Some(at)
                // otherwise, is the span a resource? in that case, we also want
                // to enter it, although we don't care about recording poll
                // stats.
            } else if exts.get::<Arc<stats::ResourceStats>>().is_some() {
                Some(at.unwrap_or_else(Instant::now))
            } else {
                None
            }
        }

        if let Some(span) = cx.span(id) {
            if let Some(now) = update(&span, None) {
                if let Some(parent) = span.parent() {
                    update(&parent, Some(now));
                }
                self.current_spans
                    .get_or_default()
                    .borrow_mut()
                    .push(id.clone());
            }
        }
    }

    fn on_exit(&self, id: &span::Id, cx: Context<'_, S>) {
        fn update<S: Subscriber + for<'a> LookupSpan<'a>>(
            span: &SpanRef<S>,
            at: Option<Instant>,
        ) -> Option<Instant> {
            let exts = span.extensions();
            // if the span we are entering is a task or async op, record the
            // poll stats.
            if let Some(stats) = exts.get::<Arc<stats::TaskStats>>() {
                let at = at.unwrap_or_else(Instant::now);
                stats.end_poll(at);
                Some(at)
            } else if let Some(stats) = exts.get::<Arc<stats::AsyncOpStats>>() {
                let at = at.unwrap_or_else(Instant::now);
                stats.end_poll(at);
                Some(at)
                // otherwise, is the span a resource? in that case, we also want
                // to enter it, although we don't care about recording poll
                // stats.
            } else if exts.get::<Arc<stats::ResourceStats>>().is_some() {
                Some(at.unwrap_or_else(Instant::now))
            } else {
                None
            }
        }

        if let Some(span) = cx.span(id) {
            if let Some(now) = update(&span, None) {
                if let Some(parent) = span.parent() {
                    update(&parent, Some(now));
                }
                self.current_spans.get_or_default().borrow_mut().pop(id);
            }
        }
    }

    fn on_close(&self, id: span::Id, cx: Context<'_, S>) {
        if let Some(span) = cx.span(&id) {
            let now = Instant::now();
            let exts = span.extensions();
            if let Some(stats) = exts.get::<Arc<stats::TaskStats>>() {
                stats.drop_task(now);
            } else if let Some(stats) = exts.get::<Arc<stats::AsyncOpStats>>() {
                stats.drop_async_op(now);
            } else if let Some(stats) = exts.get::<Arc<stats::ResourceStats>>() {
                stats.drop_resource(now);
            }
        }
    }
}
