use crossbeam_channel::{Sender, TrySendError};
use std::any::TypeId;
use std::cell::RefCell;
use std::net::IpAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use thread_local::ThreadLocal;
use tracing_core::span::{Attributes, Id, Record};
use tracing_core::{Interest, LevelFilter, Metadata, Subscriber};
use tracing_subscriber::filter::Filtered;
use tracing_subscriber::layer::{Context, Filter, Layered};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Layer;

mod aggregate;
mod callsites;
mod event;
mod server;
mod stack;

use crate::aggregate::Aggregator;
use crate::callsites::Callsites;
use event::Event;
pub use server::Server;
use stack::SpanStack;

pub struct ConsoleLayer {
    current_spans: ThreadLocal<RefCell<SpanStack>>,

    tx: Sender<Event>,
    shared: Arc<Shared>,

    spawn_callsites: Callsites<8>,
    waker_callsites: Callsites<8>,
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
        }
    }
}

#[derive(Debug, Default)]
struct Shared {
    dropped_tasks: AtomicUsize,
    dropped_resources: AtomicUsize,
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
        let aggregator = Aggregator::new(events);
        let server = Server::new(aggregator);
        let layer = Self {
            current_spans: ThreadLocal::new(),
            tx,
            shared,
            spawn_callsites: Callsites::default(),
            waker_callsites: Callsites::default(),
        };

        (layer, server)
    }
}

impl ConsoleLayer {
    const DEFAULT_EVENT_BUFFER_CAPACITY: usize = 1024;
    const DEFAULT_CLIENT_BUFFER_CAPACITY: usize = 1024;

    fn is_spawn(&self, metadata: &Metadata<'static>) -> bool {
        self.spawn_callsites.contains(metadata)
    }

    fn is_waker(&self, metadata: &Metadata<'static>) -> bool {
        self.waker_callsites.contains(metadata)
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
            (_, "executor::spawn") => {
                self.spawn_callsites.insert(metadata);
                &self.shared.dropped_tasks
            }
            (_, "executor::waker") => {
                self.waker_callsites.insert(metadata);
                &self.shared.dropped_tasks
            }
            (_, _) => &self.shared.dropped_tasks,
        };

        self.send_metadata(dropped, Event::Metadata(metadata));

        Interest::always()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
