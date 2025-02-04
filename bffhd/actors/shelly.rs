use futures_util::future::BoxFuture;
use std::collections::HashMap;

use crate::actors::Actor;
use crate::db::ArchivedValue;
use crate::resources::modules::fabaccess::ArchivedStatus;
use crate::resources::state::State;
use rumqttc::{AsyncClient, QoS};

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

        Shelly {
            name,
            client,
            topic,
        }
    }

    /// Set the name to a new one. This changes the shelly that will be activated
    pub fn set_name(&mut self, new_name: String) {
        tracing::debug!(old=%self.name, new=%new_name, "Renaming shelly actor");
        self.name = new_name;
    }
}

impl Actor for Shelly {
    fn apply(&mut self, state: ArchivedValue<State>) -> BoxFuture<'static, ()> {
        tracing::debug!(?state, name=%self.name,
            "Shelly changing state"
        );
        let pl = match state.as_ref().inner.state {
            ArchivedStatus::InUse(_) => "on",
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
