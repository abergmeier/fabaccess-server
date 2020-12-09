use std::pin::Pin;
use std::task::{Poll, Context};
use std::sync::Arc;
use std::collections::HashMap;
use std::future::Future;

use smol::Executor;

use futures::{future::BoxFuture, Stream, StreamExt};
use futures::channel::mpsc;
use futures_signals::signal::{Signal, MutableSignalCloned, MutableSignal, Mutable};

use crate::db::machine::MachineState;
use crate::config::Settings;
use crate::error::Result;

use crate::network::ActorMap;

pub trait Actuator {
    fn apply(&mut self, state: MachineState) -> BoxFuture<'static, ()>;
}

pub type ActorSignal = Box<dyn Signal<Item=MachineState> + Unpin + Send>;

pub struct Actor {
    // FIXME: This should really be a Signal.
    // But, alas, MutableSignalCloned is itself not `Clone`. For good reason as keeping track of
    // the changes itself happens in a way that Clone won't work (well).
    // So, you can't clone it, you can't copy it and you can't get at the variable inside outside
    // of a task context. In short, using Mutable isn't possible and we would have to write our own
    // implementation of MutableSignal*'s . Preferably with the correct optimizations for our case
    // where there is only one consumer. So a mpsc channel that drops all but the last input.
    rx: mpsc::Receiver<Option<ActorSignal>>,
    inner: Option<ActorSignal>,

    actuator: Box<dyn Actuator + Send + Sync>,
    future: Option<BoxFuture<'static, ()>>,
}

impl Actor {
    pub fn new(rx: mpsc::Receiver<Option<ActorSignal>>, actuator: Box<dyn Actuator + Send + Sync>) -> Self {
        Self { 
            rx: rx,
            inner: None,
            actuator: actuator,
            future: None,
        }
    }

    pub fn wrap(actuator: Box<dyn Actuator + Send + Sync>) -> (mpsc::Sender<Option<ActorSignal>>, Self) {
        let (tx, rx) = mpsc::channel(1);
        (tx, Self::new(rx, actuator))
    }
}

impl Future for Actor {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let mut this = &mut *self;
        let mut done = false; // Is the channel with new state-signals exhausted?

        // Update the signal we're polling from, if there is an update that is.
        match Stream::poll_next(Pin::new(&mut this.rx), cx) {
            Poll::Ready(None) => done = true,
            Poll::Ready(Some(new_signal)) => this.inner = new_signal,
            Poll::Pending => { },
        }

        // Poll the `apply` future.
        match this.future.as_mut().map(|future| Future::poll(Pin::new(future), cx)) {
            None => { }
            Some(Poll::Ready(_)) => this.future = None,
            Some(Poll::Pending) => return Poll::Pending,
        }

        // Poll the signal and apply all changes that happen to the inner Actuator
        match this.inner.as_mut().map(|inner| Signal::poll_change(Pin::new(inner), cx)) {
            None => Poll::Pending,
            Some(Poll::Pending) => Poll::Pending,
            Some(Poll::Ready(None)) => {
                this.inner = None;

                if done {
                    Poll::Ready(())
                } else {
                    Poll::Pending
                }
            },
            Some(Poll::Ready(Some(state))) => {
                this.future.replace(this.actuator.apply(state));
                Poll::Pending
            }
        }
    }
}

pub struct Dummy;

impl Actuator for Dummy {
    fn apply(&mut self, state: MachineState) -> BoxFuture<'static, ()> {
        println!("New state for dummy actuator: {:?}", state);
        Box::pin(smol::future::ready(()))
    }
}

pub fn load() -> Result<(ActorMap, Vec<Actor>)> {
    let d = Box::new(Dummy);
    let (tx, a) = Actor::wrap(d);

    let mut map = HashMap::new();
    map.insert("Dummy".to_string(), tx);

    Ok(( map, vec![a] ))
}
