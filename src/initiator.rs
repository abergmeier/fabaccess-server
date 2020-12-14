use std::pin::Pin;
use std::task::{Poll, Context};
use std::future::Future;
use std::collections::HashMap;

use smol::{Task, Timer};

use slog::Logger;

use paho_mqtt::AsyncClient;

use futures::FutureExt;
use futures::future::BoxFuture;

use genawaiter::{sync::{Gen, GenBoxed, Co}, GeneratorState};

use futures_signals::signal::{Signal, Mutable, MutableSignalCloned};
use crate::machine::{Machine, ReturnToken};
use crate::db::machine::MachineState;
use crate::db::user::{User, UserId, UserData};

use crate::network::InitMap;

use crate::error::Result;
use crate::config::Config;

pub trait Sensor {
    fn run_sensor(&mut self) -> BoxFuture<'static, (Option<User>, MachineState)>;
}

type BoxSensor = Box<dyn Sensor + Send>;

pub struct Initiator {
    signal: MutableSignalCloned<Option<Machine>>,
    machine: Option<Machine>,
    future: Option<BoxFuture<'static, (Option<User>, MachineState)>>,
    token: Option<ReturnToken>,
    sensor: BoxSensor,
}

impl Initiator {
    pub fn new(sensor: BoxSensor, signal: MutableSignalCloned<Option<Machine>>) -> Self {
        Self {
            signal: signal,
            machine: None,
            future: None,
            token: None,
            sensor: sensor,
        }
    }

    pub fn wrap(sensor: BoxSensor) -> (Mutable<Option<Machine>>, Self) {
        let m = Mutable::new(None);
        let s = m.signal_cloned();

        (m, Self::new(sensor, s))
    }
}

impl Future for Initiator {
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
                    this.future = Some(this.sensor.run_sensor());
                },
                Some(Poll::Ready((user, state))) => {
                    this.future.take();
                    this.machine.as_mut().map(|machine| machine.request_state_change(user.as_ref(), state).unwrap());
                }
                Some(Poll::Pending) => return Poll::Pending,
            }
        }
    }
}

pub fn load(log: &Logger, client: &AsyncClient, config: &Config) -> Result<(InitMap, Vec<Initiator>)> {
    let mut map = HashMap::new();

    let initiators = config.initiators.iter()
        .map(|(k,v)| (k, load_single(log, client, k, &v.module, &v.params)))
        .filter_map(|(k,n)| match n {
            None => None,
            Some(i) => Some((k, i)),
        });

    let mut v = Vec::new();
    for (name, initiator) in initiators {
        let (m, i) = Initiator::wrap(initiator);
        map.insert(name.clone(), m);
        v.push(i);
    }

    Ok((map, v))
}

fn load_single(
    log: &Logger,
    client: &AsyncClient,
    name: &String,
    module_name: &String,
    params: &HashMap<String, String>
    ) -> Option<BoxSensor>
{
    match module_name.as_ref() {
        "Dummy" => {
            Some(Box::new(Dummy::new(log)))
        },
        _ => {
            error!(log, "No initiator found with name \"{}\", configured as \"{}\"", 
                module_name, name);
            None
        }
    }
}

pub struct Dummy {
    log: Logger,
    step: bool,
}

impl Dummy {
    pub fn new(log: &Logger) -> Self {
        Self { log: log.new(o!("module" => "Dummy Initiator")), step: false }
    }
}

impl Sensor for Dummy {
    fn run_sensor(&mut self)
        -> BoxFuture<'static, (Option<User>, MachineState)>
    {
        let step = self.step;
        self.step = !step;

        info!(self.log, "Kicking off new dummy initiator state change: {}", step);

        let f = async move {
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
        };

        Box::pin(f)
    }
}

