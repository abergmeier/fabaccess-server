use slog::Logger;

use std::sync::Arc;
use smol::lock::RwLock;

use std::pin::Pin;
use futures::ready;
use futures::prelude::*;
use futures::channel::mpsc;
use futures::task::{Context, Poll, Spawn};
use futures_signals::signal::Signal;

use crate::db::machine::Status;

use std::collections::HashMap;

#[derive(Clone)]
pub struct Actuators {
    inner: Arc<RwLock<Inner>>,
}

pub type ActBox = Box<dyn Actuator + Sync + Send + Unpin>;

type Inner = HashMap<String, mpsc::Sender<StatusSignal>>;

impl Actuators {
    pub fn new() -> Self {
        Actuators {
            inner: Arc::new(RwLock::new(Inner::new()))
        }
    }

    pub async fn register(&self, name: String, tx: mpsc::Sender<StatusSignal>) {
        let mut wlock = self.inner.write().await;
        // TODO: Log an error or something if that name was already taken
        wlock.insert(name, tx);
    }

    pub async fn subscribe(&mut self, name: String, signal: StatusSignal) {
        let mut wlock = self.inner.write().await;
        if let Some(tx) = wlock.get_mut(&name) {
            tx.send(signal).await;
        }
    }
}

pub type StatusSignal = Pin<Box<dyn Signal<Item = Status> + Send + Sync>>;

pub trait Actuator: Stream<Item = future::BoxFuture<'static, ()>> {
    fn subscribe(&mut self, signal: StatusSignal);
}

// This is merely a proof that Actuator *can* be implemented on a finite, known type. Yay for type
// systems with halting problems.
struct Dummy {
    log: Logger,
    sigchan: mpsc::Receiver<StatusSignal>,
    signal: Option<StatusSignal>,
}

impl Actuator for Dummy {
    fn subscribe(&mut self, signal: StatusSignal) {
        self.signal.replace(signal);
    }
}

impl Stream for Dummy {
    type Item = future::BoxFuture<'static, ()>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let unpin = Pin::into_inner(self);
        if let Some(ref mut s) = unpin.signal {
            let status = ready!(Signal::poll_change(Pin::new(s), cx));

            info!(unpin.log, "Dummy actuator would set status to {:?}, but is a Dummy", status);

            Poll::Ready(Some(Box::pin(futures::future::ready(()))))
        } else {
            Poll::Pending
        }
    }
}
