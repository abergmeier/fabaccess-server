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

use crate::registries::sensors::Sensor;

use crate::error::Result;

pub struct Initiator<S: Sensor> {
    signal: MutableSignalCloned<Option<Machine>>,
    machine: Option<Machine>,
    future: Option<BoxFuture<'static, (S::State, Option<User>, MachineState)>>,
    token: Option<ReturnToken>,
    //state: Option<S::State>,
    sensor: Box<S>,
}

impl<S: Sensor> Initiator<S> {
    pub fn new(sensor: Box<S>, signal: MutableSignalCloned<Option<Machine>>) -> Self {
        Self {
            signal: signal,
            machine: None,
            future: None,
            token: None,
            //state: None,
            sensor: sensor,
        }
    }
}

impl<S: Sensor> Future for Initiator<S> {
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
                    this.future = Some(this.sensor.run_sensor(None));
                },
                Some(Poll::Ready((fut_state, user, state))) => {
                    this.future.take();
                    //this.state.replace(fut_state);
                    this.machine.as_mut().map(|machine| machine.request_state_change(user.as_ref(), state));
                }
                Some(Poll::Pending) => return Poll::Pending,
            }
        }
    }
}

pub fn load<S: Sensor>() -> Result<Initiator<S>> {
    unimplemented!()
}

pub struct Dummy;

impl Sensor for Dummy {
    type State = bool;

    fn run_sensor(&mut self, state: Option<bool>)
        -> BoxFuture<'static, (Self::State, Option<User>, MachineState)>
    {
        let step = state.map(|b| !b).unwrap_or(false);
        let f = async move {
            Timer::after(std::time::Duration::from_secs(1)).await;
            if step {
                return (step, None, MachineState::free());
            } else {
                let user = User::new(
                    UserId::new("test".to_string(), None, None),
                    UserData::new(vec![], 0),
                );
                let p = user.data.priority;
                let id = user.id.clone();
                return (step, Some(user), MachineState::used(id, p));
            }
        };

        Box::pin(f)
    }
}

