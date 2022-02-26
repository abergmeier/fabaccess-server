use std::pin::Pin;
use std::task::{Poll, Context};
use std::future::Future;
use std::collections::HashMap;

use smol::Timer;

use slog::Logger;



use futures::future::BoxFuture;

use futures_signals::signal::{Signal, Mutable, MutableSignalCloned};
use crate::machine::Machine;
use crate::db::machine::MachineState;
use crate::db::user::{UserId};

use crate::network::InitMap;

use crate::error::Result;
use crate::config::Config;

pub trait Sensor {
    fn run_sensor(&mut self) -> BoxFuture<'static, (Option<UserId>, MachineState)>;
}

type BoxSensor = Box<dyn Sensor + Send>;

pub struct Initiator {
    log: Logger,
    signal: MutableSignalCloned<Option<Machine>>,
    machine: Option<Machine>,
    future: Option<BoxFuture<'static, (Option<UserId>, MachineState)>>,
    // TODO: Prepare the init for async state change requests.
    state_change_fut: Option<BoxFuture<'static, Result<()>>>,
    sensor: BoxSensor,
}

impl Initiator {
    pub fn new(log: Logger, sensor: BoxSensor, signal: MutableSignalCloned<Option<Machine>>) -> Self {
        Self {
            log: log,
            signal: signal,
            machine: None,
            future: None,
            state_change_fut: None,
            sensor: sensor,
        }
    }

    pub fn wrap(log: Logger, sensor: BoxSensor) -> (Mutable<Option<Machine>>, Self) {
        let m = Mutable::new(None);
        let s = m.signal_cloned();

        (m, Self::new(log, sensor, s))
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
            Poll::Ready(Some(machine)) => {

                match machine.as_ref().map(|m| m.try_lock()) {
                    None => info!(this.log, "Deinstalled machine"),
                    Some(None) => info!(this.log, "Installed new machine with locked mutex!"),
                    Some(Some(g)) => info!(this.log, "Installed new machine {}", g.id),
                }

                this.machine = machine;
            },
        }

        // Do as much work as we can:
        loop {
            // Always poll the state change future first
            if let Some(ref mut f) = this.state_change_fut {
                match Future::poll(Pin::new(f), cx) {
                    // If there is a state change future and it would block we return early
                    Poll::Pending => {
                        debug!(this.log, "State change blocked");
                        return Poll::Pending;
                    },
                    Poll::Ready(Ok(_rt)) => {
                        debug!(this.log, "State change returned ok");
                        // Explicity drop the future
                        let _ = this.state_change_fut.take();

                    }
                    Poll::Ready(Err(e)) => {
                        info!(this.log, "State change returned err: {}", e);
                        // Explicity drop the future
                        let _ = this.state_change_fut.take();
                    }
                }
            }

            // If there is a future, poll it
            match this.future.as_mut().map(|future| Future::poll(Pin::new(future), cx)) {
                None => {
                    this.future = Some(this.sensor.run_sensor());
                },
                Some(Poll::Ready((user, state))) => {
                    debug!(this.log, "Sensor returned a new state");
                    this.future.take();
                    let f = this.machine.as_mut().map(|machine| {
                        machine.request_state_change(user.as_ref(), state).unwrap()
                    });
                    this.state_change_fut = f;
                }
                Some(Poll::Pending) => return Poll::Pending,
            }
        }
    }
}

pub fn load(log: &Logger, config: &Config) -> Result<(InitMap, Vec<Initiator>)> {
    let mut map = HashMap::new();

    let initiators = config.initiators.iter()
        .map(|(k,v)| (k, load_single(log, k, &v.module, &v.params)))
        .filter_map(|(k,n)| match n {
            None => None,
            Some(i) => Some((k, i)),
        });

    let mut v = Vec::new();
    for (name, initiator) in initiators {
        let (m, i) = Initiator::wrap(log.new(o!("name" => name.clone())), initiator);
        map.insert(name.clone(), m);
        v.push(i);
    }

    Ok((map, v))
}

fn load_single(
    log: &Logger,
    name: &String,
    module_name: &String,
    params: &HashMap<String, String>
    ) -> Option<BoxSensor>
{
    match module_name.as_ref() {
        "Dummy" => {
            Some(Box::new(Dummy::new(log, params)))
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
    userid: Option<UserId>,
}

impl Dummy {
    pub fn new(log: &Logger, params: &HashMap<String, String>) -> Self {
        let userid = if let Some(uid) = params.get("uid") {
            Some(UserId::new(uid.clone(),
                             params.get("subuid").map(String::from),
                             params.get("realm").map(String::from)
            ))
        } else {
            None
        };

        let log = log.new(o!("module" => "Dummy Initiator"));
        debug!(log, "Constructed dummy initiator with params: {:?}", params);

        Self { log, step: false, userid }

    }
}

impl Sensor for Dummy {
    fn run_sensor(&mut self)
        -> BoxFuture<'static, (Option<UserId>, MachineState)>
    {
        let step = self.step;
        self.step = !step;

        info!(self.log, "Kicking off new dummy initiator state change: {}, {:?}",
            if step { "free" } else { "used" },
            &self.userid
        );

        let userid = self.userid.clone();
        let f = async move {
            Timer::after(std::time::Duration::from_secs(1)).await;
            if step {
                return (userid.clone(), MachineState::free());
            } else {
                return (userid.clone(), MachineState::used(userid.clone()));
            }
        };

        Box::pin(f)
    }
}

