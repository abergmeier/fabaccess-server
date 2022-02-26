use std::collections::HashMap;
use slog::Logger;

use crate::db::machine::Status;

use futures::future::BoxFuture;
use rumqttc::{AsyncClient, QoS};

use crate::actor::Actuator;
use crate::db::machine::MachineState;

/// An actuator for a Shellie connected listening on one MQTT broker
///
/// This actuator will toggle the shellie with the given `name`.
/// If you need to toggle shellies on multiple brokers you need multiple instanced of this
/// actuator with different clients.
pub struct Shelly {
    log: Logger,
    name: String,
    client: AsyncClient,
    topic: String,
}

impl Shelly {
    pub fn new(log: Logger, name: String, client: AsyncClient, params: &HashMap<String, String>) -> Self {
        let topic = if let Some(topic) = params.get("topic") {
            format!("shellies/{}/relay/0/command", topic)
        } else {
            format!("shellies/{}/relay/0/command", name)
        };
        debug!(log,
            "Starting shelly module for {name} with topic '{topic}'",
            name = &name,
            topic = &topic,
        );

        Shelly { log, name, client, topic, }
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
        let pl = match state.state {
            Status::InUse(_) => "on",
            _ => "off",
        };

        let elog = self.log.clone();
        let name = self.name.clone();
        let client = self.client.clone();
        let topic = self.topic.clone();
        let f = async move {
            let res = client.publish(topic, QoS::AtLeastOnce, false, pl).await;
            if let Err(e) = res {
                error!(elog,"Shelly actor {} failed to update state: {:?}",name,e,);
            }
        };

        return Box::pin(f);
    }
}
