use slog::Logger;

use crate::config::Settings;
use crate::error::Result;
use crate::db::machine::Status;

use std::pin::Pin;
use futures::prelude::*;
use futures::channel::mpsc;
use futures::future::BoxFuture;
use futures::ready;
use futures::task::{Poll, Context, Waker, Spawn, FutureObj};
use futures::StreamExt;
use futures_signals::signal::Signal;

use crate::actor::Actuator;
use crate::db::machine::MachineState;

use paho_mqtt as mqtt;

/// An actuator for a Shellie connected listening on one MQTT broker
///
/// This actuator will toggle the shellie with the given `name`.
/// If you need to toggle shellies on multiple brokers you need multiple instanced of this
/// actuator with different clients.
pub struct Shelly {
    log: Logger,
    name: String,
    client: mqtt::AsyncClient,
}

impl Shelly {
    pub fn new(log_view: &Logger, name: String, client: mqtt::AsyncClient) -> Self {
        let log = log_view.new(o!("shelly_name" => name.clone()));
        debug!(log, "Starting shelly module for {}", &name);
        Shelly { log, name, client, }
    }

    /// Set the name to a new one. This changes the shelly that will be activated
    pub fn set_name(&mut self, new_name: String) {
        let log = self.log.new(o!("shelly_name" => new_name.clone()));
        self.name = new_name;
        self.log = log;
    }
}


impl Actuator for Shelly {
    fn apply(&mut self, state: MachineState) -> BoxFuture<'static, ()> {
        info!(self.log, "Machine Status changed: {:?}", state);
        let topic = format!("shellies/{}/relay/0/command", self.name);
        let pl = match state.state {
            Status::InUse(_, _) => "on",
            _ => "off",
        };
        let msg = mqtt::Message::new(topic, pl, 0);
        let f = self.client.publish(msg).map(|_| ());

        return Box::pin(f);
    }
}
