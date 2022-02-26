use std::pin::Pin;
use std::task::{Poll, Context};
use std::sync::Mutex;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::future::Future;
use std::time::Duration;

use futures::{future::BoxFuture, Stream};
use futures::channel::mpsc;
use futures_signals::signal::Signal;
use rumqttc::{AsyncClient, ConnectionError, Event, Incoming, MqttOptions, OptionError, Transport};

use crate::db::machine::MachineState;
use crate::config::Config;
use crate::error::Result;
use crate::network::ActorMap;

use slog::Logger;
use url::Url;
use crate::Error;
use crate::Error::{BadConfiguration, MQTTConnectionError};

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

        // FIXME: This is potentially invalid, and may lead to the situation that the signal is
        // replaced *twice* but the second change will not be honoured since this implementation of
        // events is *EDGE*-triggered!
        // Update the signal we're polling from, if there is an update that is.
        match Stream::poll_next(Pin::new(&mut this.rx), cx) {
            Poll::Ready(None) => done = true,
            Poll::Ready(Some(new_signal)) => this.inner = new_signal,
            Poll::Pending => { },
        }

        // Work until there is no more work to do.
        loop {

            // Poll the `apply` future. And ensure it's completed before the next one is started
            match this.future.as_mut().map(|future| Future::poll(Pin::new(future), cx)) {
                // Skip and poll for a new future to do
                None => { }

                // This apply future is done, get a new one
                Some(Poll::Ready(_)) => this.future = None,

                // This future would block so we return to continue work another time
                Some(Poll::Pending) => return Poll::Pending,
            }

            // Poll the signal and apply any change that happen to the inner Actuator
            match this.inner.as_mut().map(|inner| Signal::poll_change(Pin::new(inner), cx)) {
                // No signal to poll
                None => return Poll::Pending,
                Some(Poll::Pending) => return Poll::Pending,
                Some(Poll::Ready(None)) => {
                    this.inner = None;

                    if done {
                        return Poll::Ready(());
                    } else {
                        return Poll::Pending;
                    }
                },
                Some(Poll::Ready(Some(state))) => {
                    // This future MUST be polled before we exit from the Actor::poll because if we
                    // do not do that it will not register the dependency and thus NOT BE POLLED.
                    this.future.replace(this.actuator.apply(state));
                }
            }
        }
    }
}

pub struct Dummy {
    log: Logger,
}

impl Dummy {
    pub fn new(log: Logger) -> Self {
        Self { log }
    }
}

impl Actuator for Dummy {
    fn apply(&mut self, state: MachineState) -> BoxFuture<'static, ()> {
        info!(self.log, "New state for dummy actuator: {:?}", state);
        Box::pin(smol::future::ready(()))
    }
}

pub fn load(log: &Logger, config: &Config) -> Result<(ActorMap, Vec<Actor>)> {
    let mut map = HashMap::new();

    let mqtt_url = Url::parse(config.mqtt_url.as_str())?;
    let mut mqttoptions = MqttOptions::try_from(mqtt_url)
        .map_err(|opt| Error::Boxed(Box::new(opt)))?;
    mqttoptions.set_keep_alive(Duration::from_secs(20));

    let (mut mqtt, mut eventloop) = AsyncClient::new(mqttoptions, 256);
    let dlog = log.clone();
    let mut eventloop = smol::block_on(async move {
        match eventloop.poll().await {
            Ok(Event::Incoming(Incoming::Connect(connect))) => {},
            Ok(e) => {
                warn!(dlog, "Got unexpected mqtt event {:?}", e);
            }
            Err(connerror) => {
                error!(dlog, "MQTT connection failed: {:?}", &connerror);
                return Err(MQTTConnectionError(connerror));
            }
        }

        Ok(eventloop)
    })?;
    let dlog = log.clone();
    smol::spawn(async move {
        let mut fault = false;
        loop {
            match eventloop.poll().await {
                Ok(_) => {
                    fault = false;
                    // TODO: Handle incoming MQTT messages
                }
                Err(ConnectionError::Cancel) |
                Err(ConnectionError::StreamDone) |
                Err(ConnectionError::RequestsDone) => {
                    // Normal exit
                    info!(dlog, "MQTT request queue closed, stopping client.");
                    return;
                }
                Err(ConnectionError::Timeout(_)) => {
                    error!(dlog, "MQTT operation timed out!");
                    warn!(dlog, "MQTT client will continue, but messages may have been lost.")
                    // Timeout does not close the client
                }
                Err(ConnectionError::Io(e)) if fault => {
                    error!(dlog, "MQTT recurring IO error, closing client: {}", e);
                    // Repeating IO errors close client. Any Ok() in between resets fault to false.
                    return;
                }
                Err(ConnectionError::Io(e)) => {
                    fault = true;
                    error!(dlog, "MQTT encountered IO error: {}", e);
                    // *First* IO error does not close the client.
                }
                Err(e) => {
                   error!(dlog, "MQTT client encountered unhandled error: {:?}", e);
                   return;
                }
            }
        }
    }).detach();

    let actuators = config.actors.iter()
        .map(|(k,v)| (k, load_single(log, k, &v.module, &v.params, mqtt.clone())))
        .filter_map(|(k, n)| match n {
            None => None,
            Some(a) => Some((k, a))
        });

    let mut v = Vec::new();
    for (name, actuator) in actuators {
        let (tx, a) = Actor::wrap(actuator);
        map.insert(name.clone(), Mutex::new(tx));
        v.push(a);
    }


    Ok(( map, v ))
}

fn load_single(
    log: &Logger, 
    name: &String,
    module_name: &String,
    params: &HashMap<String, String>,
    client: AsyncClient,
    ) -> Option<Box<dyn Actuator + Sync + Send>> 
{
    use crate::modules::*;

    info!(log, "Loading actor \"{}\" with module {} and params {:?}", name, module_name, params);
    let log = log.new(o!("name" => name.clone()));
    match module_name.as_ref() {
        "Dummy" => {
            Some(Box::new(Dummy::new(log)))
        }
        "Process" => {
            Process::new(log, name.clone(), params)
                .map(|a| a.into_boxed_actuator())
        }
        "Shelly" => {
            Some(Box::new(Shelly::new(log, name.clone(), client, params)))
        }
        _ => {
            error!(log, "No actor found with name \"{}\", configured as \"{}\".", module_name, name);
            None
        },
    }
}
