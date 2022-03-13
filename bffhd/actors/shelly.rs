use std::collections::HashMap;
use futures_util::future::BoxFuture;
use rumqttc::{AsyncClient, QoS};
use crate::actors::Actor;
use crate::resources::modules::fabaccess::Status;
use crate::resources::state::State;

/// An actuator for a Shellie connected listening on one MQTT broker
///
/// This actuator will toggle the shellie with the given `name`.
/// If you need to toggle shellies on multiple brokers you need multiple instanced of this
/// actuator with different clients.
pub struct Shelly {
    name: String,
    client: AsyncClient,
    topic: String,
}

impl Shelly {
    pub fn new(name: String, client: AsyncClient, params: &HashMap<String, String>) -> Self {
        let topic = if let Some(topic) = params.get("topic") {
            format!("shellies/{}/relay/0/command", topic)
        } else {
            format!("shellies/{}/relay/0/command", name)
        };

        tracing::debug!(%name,%topic,"Starting shelly module");

        Shelly { name, client, topic, }
    }

    /// Set the name to a new one. This changes the shelly that will be activated
    pub fn set_name(&mut self, new_name: String) {
        tracing::debug!(old=%self.name, new=%new_name, "Renaming shelly actor");
        self.name = new_name;
    }
}


impl Actor for Shelly {
    fn apply(&mut self, state: State) -> BoxFuture<'static, ()> {
        tracing::debug!(?state, "Shelly changing state");
        let pl = match state.inner.state {
            Status::InUse(_) => "on",
            _ => "off",
        };

        let name = self.name.clone();
        let client = self.client.clone();
        let topic = self.topic.clone();
        let f = async move {
            let res = client.publish(topic, QoS::AtLeastOnce, false, pl).await;
            if let Err(error) = res {
                tracing::error!(?error, %name, "`Shelly` actor failed to update state");
            }
        };

        return Box::pin(f);
    }
}
