use std::cell::Cell;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use futures_signals::signal::{MutableSignalRef, ReadOnlyMutable, Signal};
use futures_util::future::BoxFuture;
use crate::resource::state::State;

pub trait Actor {
    fn apply(&mut self, state: State) -> BoxFuture<'static, ()>;
}

fn loader<S: Signal<Item = State>>(cell: &Cell<Option<S>>) -> Option<S> {
    cell.take()
}

pub struct ActorDriver<S: 'static> {
    rx: MutableSignalRef<Cell<Option<S>>, &'static dyn Fn(&Cell<Option<S>>) -> Option<S>>,
    signal: Option<S>,

    actor: Box<dyn Actor + Send + Sync>,
    future: Option<BoxFuture<'static, ()>>,
}

impl<S: Signal<Item = State>> ActorDriver<S>
{
    pub fn new(rx: &ReadOnlyMutable<Cell<Option<S>>>, actor: Box<dyn Actor + Send + Sync>)
        -> Self
    {
        let f: &'static dyn Fn(&Cell<Option<S>>) -> Option<S> = &loader;
        let rx = rx.signal_ref(f);
        Self {
            rx,
            signal: None,
            actor,
            future: None,
        }
    }
}


impl<S> Future for ActorDriver<S>
    where S: Signal<Item=State> + Unpin + Send,
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let mut done = false; // Is the channel with new state-signals exhausted?

        match Pin::new(&mut self.rx).poll_change(cx) {
            Poll::Ready(Some(new_signal)) => { self.signal = new_signal; },
            Poll::Ready(None) => done = true,
            Poll::Pending => {},
        }

        // Work until there is no more work to do.
        loop {

            // Poll the `apply` future. And ensure it's completed before the next one is started
            match self.future.as_mut()
                .map(|future| Future::poll(Pin::new(future), cx))
            {
                // Skip and poll for a new future to do
                None => { }

                // This apply future is done, get a new one
                Some(Poll::Ready(_)) => self.future = None,

                // This future would block so we return to continue work another time
                Some(Poll::Pending) => return Poll::Pending,
            }

            // Poll the signal and apply any change that happen to the inner Actuator
            match self.signal.as_mut()
                .map(|inner| S::poll_change(Pin::new(inner), cx))
            {
                // No signal to poll
                None => return Poll::Pending,
                Some(Poll::Pending) => return Poll::Pending,
                Some(Poll::Ready(None)) => {
                    self.signal = None;

                    if done {
                        return Poll::Ready(());
                    } else {
                        return Poll::Pending;
                    }
                },
                Some(Poll::Ready(Some(state))) => {
                    // This future MUST be polled before we exit from the Actor::poll because if we
                    // do not do that it will not register the dependency and thus NOT BE POLLED.
                    let f = self.actor.apply(state);
                    self.future.replace(f);
                }
            }
        }
    }
}

