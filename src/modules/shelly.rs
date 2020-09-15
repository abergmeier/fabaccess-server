use slog::Logger;

use crate::config::Config;
use crate::registries::{Registries, Actuator, ActBox};
use crate::error::Result;

use std::pin::Pin;
use futures::prelude::*;
use futures::ready;
use futures::task::{Poll, Context};

use paho_mqtt as mqtt;

// TODO: Late config parsing. Right now the config is validated at the very startup in its
// entirety. This works reasonably enough for this static modules here but if we do dynamic loading
// via dlopen(), lua API, python API etc it will not.
pub async fn run(log: Logger, config: Config, registries: Registries) {
    let shelly_r = Shelly::new(config).await;
    if let Err(e) = shelly_r {
        error!(log, "Shelly module errored: {}", e);
        return;
    }

    let r = registries.actuators.register(
        "shelly".to_string(), 
        shelly_r.unwrap()
    ).await;
}

/// An actuator for all Shellies connected listening on one MQTT broker
///
/// This actuator can power toggle an arbitrariy named shelly on the broker it is connected to. If
/// you need to toggle shellies on multiple brokers you need multiple instanced of this actuator.
struct Shelly {
    client: mqtt::AsyncClient,
}

impl Shelly {
    pub async fn new(config: Config) -> Result<ActBox> {
        let client = mqtt::AsyncClient::new(config.mqtt_url)?;

        client.connect(mqtt::ConnectOptions::new()).await?;

        Ok(Box::new(Shelly { client }) as ActBox)
    }
}


#[async_trait]
impl Actuator for Shelly {
    async fn power_on(&mut self, name: String) {
        let topic = "";
        let msg = mqtt::Message::new(topic, "1", 0);
        self.client.publish(msg).map(|_| ()).await
    }

    async fn power_off(&mut self, name: String) {
        let topic = "";
        let msg = mqtt::Message::new(topic, "0", 0);
        self.client.publish(msg).map(|_| ()).await
    }
}
