use std::pin::Pin;
use std::task::{Poll, Context};
use std::future::Future;
use smol::{Task, Timer};

use futures::FutureExt;
use futures::future::BoxFuture;

use genawaiter::{sync::{Gen, GenBoxed, Co}, GeneratorState};

use futures_signals::signal::{Signal, MutableSignalCloned};
use crate::machine::{Machine, ReturnToken};
use crate::db::machine::MachineState;
use crate::db::user::{User, UserId, UserData};

use crate::error::Result;

pub struct Initiator<'a> {
    signal: MutableSignalCloned<Option<Machine>>,
    machine: Option<Machine>,
    future: Option<BoxFuture<'a, (Option<User>, MachineState)>>,
    token: Option<ReturnToken>,
    step: bool,
}

async fn producer(step: bool) -> (Option<User>, MachineState) {
    Timer::after(std::time::Duration::from_secs(1)).await;
    if step {
        return (None, MachineState::free());
    } else {
        let user = User::new(
            UserId::new("test".to_string(), None, None),
            UserData::new(vec![], 0),
        );
        let p = user.data.priority;
        let id = user.id.clone();
        return (Some(user), MachineState::used(id, p));
    }
}

impl<'a> Initiator<'a> {
    pub fn new(signal: MutableSignalCloned<Option<Machine>>) -> Self {
        Self {
            signal: signal,
            machine: None,
            future: None,
            token: None,
            step: false,
        }
    }
}

impl<'a> Future for Initiator<'a> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let mut this = &mut *self;

        // First of course, see what machine we should work with.
        match Signal::poll_change(Pin::new(&mut this.signal), cx) {
            Poll::Pending => { }
            Poll::Ready(None) => return Poll::Ready(()),
            // Keep in mind this is actually an Option<Machine>
            Poll::Ready(Some(machine)) => this.machine = machine,
        }

        // Do as much work as we can:
        loop {
            // If there is a future, poll it
            match this.future.as_mut().map(|future| Future::poll(Pin::new(future), cx)) {
                None => {
                    this.future = Some(Box::pin(producer(this.step)));
                    this.step = !this.step;
                },
                Some(Poll::Ready((user, state))) => {
                    this.future.take();
                    this.machine.as_mut().map(|machine| machine.request_state_change(user.as_ref(), state));
                }
                Some(Poll::Pending) => return Poll::Pending,
            }
        }
    }
}

pub fn load<'a>() -> Result<Initiator<'a>> {
    unimplemented!()
}
