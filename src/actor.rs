use std::pin::Pin;
use std::task::{Poll, Context};
use std::sync::Arc;
use std::future::Future;

use smol::Executor;

use futures::{future::BoxFuture, Stream, StreamExt};
use futures::channel::mpsc;
use futures_signals::signal::{Signal, MutableSignalCloned, MutableSignal, Mutable};

use crate::db::machine::MachineState;
use crate::registries::Actuator;
use crate::config::Settings;
use crate::error::Result;

pub struct Actor<S: Signal> {
    // FIXME: This should really be a Signal.
    // But, alas, MutableSignalCloned is itself not `Clone`. For good reason as keeping track of
    // the changes itself happens in a way that Clone won't work (well).
    // So, you can't clone it, you can't copy it and you can't get at the variable inside outside
    // of a task context. In short, using Mutable isn't possible and we would have to write our own
    // implementation of MutableSignal*'s . Preferably with the correct optimizations for our case
    // where there is only one consumer. So a mpsc channel that drops all but the last input.
    rx: mpsc::Receiver<Option<S>>
    inner: S,
}

pub fn load() {
    let s = Mutable::new(MachineState::new());

    Ok(())
}

#[must_use = "Signals do nothing unless polled"]
pub struct MaybeFlatten<A: Signal, B: Signal> {
    signal: Option<A>,
    inner: Option<B>,
}

// Poll parent => Has inner   => Poll inner  => Output
// --------------------------------------------------------
// Some(Some(inner)) =>             => Some(value) => Some(value)
// Some(Some(inner)) =>             =>             => Pending
// Some(None)        =>             =>             => Pending
// None              => Some(inner) => Some(value) => Some(value)
// None              => Some(inner) => None        => None
// None              => Some(inner) => Pending     => Pending
// None              => None        =>             => None
// Pending           => Some(inner) => Some(value) => Some(value)
// Pending           => Some(inner) => None        => Pending
// Pending           => Some(inner) => Pending     => Pending
// Pending           => None        =>             => Pending
impl<A, B> Signal for MaybeFlatten<A, B>
    where A: Signal<Item=Option<B>> + Unpin,
          B: Signal + Unpin,
{
    type Item = B::Item;

    #[inline]
    fn poll_change(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let mut this = &mut *self;

        let done = match this.signal.as_mut().map(|signal| Signal::poll_change(Pin::new(signal), cx)) {
            None => true,
            Some(Poll::Ready(None)) => {
                this.signal = None;
                true
            },
            Some(Poll::Ready(Some(new_inner))) => {
                this.inner = new_inner;
                false
            },
            Some(Poll::Pending) => false,
        };

        match this.inner.as_mut().map(|inner| Signal::poll_change(Pin::new(inner), cx)) {
            Some(Poll::Ready(None)) => {
                this.inner = None;
            },
            Some(poll) => {
                return poll;
            },
            None => {},
        }

        if done {
            Poll::Ready(None)

        } else {
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_test::*;
    use futures_signals::signal::Signal;

    #[test]
    fn load_test() {
        let (a, s, m) = super::load().unwrap();

        let cx = task::panic_context();
        a.signal.poll_change(&mut cx);
    }
}
