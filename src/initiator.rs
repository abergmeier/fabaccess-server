use std::pin::Pin;
use std::task::{Poll, Context};
use std::future::Future;
use std::collections::HashMap;

use smol::Timer;

use slog::Logger;

use paho_mqtt::AsyncClient;

use futures::future::BoxFuture;

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
    log: Logger,
    signal: MutableSignalCloned<Option<Machine>>,
    machine: Option<Machine>,
    future: Option<BoxFuture<'static, (Option<User>, MachineState)>>,
    // TODO: Prepare the init for async state change requests.
    state_change_fut: Option<BoxFuture<'static, Result<ReturnToken>>>,
    token: Option<ReturnToken>,
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
            token: None,
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
                    Poll::Ready(Ok(tok)) => {
                        debug!(this.log, "State change returned ok");
                        // Explicity drop the future
                        let _ = this.state_change_fut.take();

                        // Store the given return token for future use
                        this.token.replace(tok);
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
                        machine.request_state_change(user.as_ref(), state)
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
    _params: &HashMap<String, String>
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
                    UserData::new(vec![crate::db::access::RoleIdentifier::local_from_str("lmdb".to_string(), "testrole".to_string())], 0),
                );
                let id = user.id.clone();
                return (Some(user), MachineState::used(Some(id)));
            }
        };

        Box::pin(f)
    }
}

