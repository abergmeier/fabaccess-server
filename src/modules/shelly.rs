use slog::Logger;

use crate::config::Settings;
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
pub async fn run(log: Logger, config: Settings, registries: Registries) {
    let shelly = Shelly::new(config).await;

    let r = registries.actuators.register("shelly".to_string(), shelly).await;
}

/// An actuator for all Shellies connected listening on one MQTT broker
///
/// This actuator can power toggle an arbitrariy named shelly on the broker it is connected to. If
/// you need to toggle shellies on multiple brokers you need multiple instanced of this actuator.
#[derive(Clone)]
struct Shelly {
    client: mqtt::AsyncClient,
}

impl Shelly {
    pub async fn new(config: Settings) -> ActBox {
        let client = mqtt::AsyncClient::new(config.shelly.unwrap().mqtt_url).unwrap();

        client.connect(mqtt::ConnectOptions::new()).await.unwrap();

        Box::new(Shelly { client })
    }
}


#[async_trait]
impl Actuator for Shelly {
    async fn power_on(&mut self, name: String) {
        let topic = format!("shellies/{}/relay/0/command", name);
        let msg = mqtt::Message::new(topic, "on", 0);
        self.client.publish(msg).map(|_| ()).await
    }

    async fn power_off(&mut self, name: String) {
        let topic = format!("shellies/{}/relay/0/command", name);
        let msg = mqtt::Message::new(topic, "off", 0);
        self.client.publish(msg).map(|_| ()).await
    }
}
