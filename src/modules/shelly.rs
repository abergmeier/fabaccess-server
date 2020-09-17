use slog::Logger;

use crate::config::Settings;
use crate::registries::{Registries, Actuator, ActBox, StatusSignal};
use crate::error::Result;
use crate::machine::Status;

use std::pin::Pin;
use futures::prelude::*;
use futures::ready;
use futures::task::{Poll, Context, Waker, Spawn};
use futures::StreamExt;
use futures_signals::signal::Signal;

use paho_mqtt as mqtt;

// TODO: Late config parsing. Right now the config is validated at the very startup in its
// entirety. This works reasonably enough for this static modules here but if we do dynamic loading
// via dlopen(), lua API, python API etc it will not.
pub async fn run(log: Logger, config: Settings, registries: Registries) {
    let shelly = Shelly::new(config).await;

    let r = registries.actuators.register("shelly".to_string(), Box::new(shelly)).await;
}

/// An actuator for all Shellies connected listening on one MQTT broker
///
/// This actuator can power toggle an arbitrariy named shelly on the broker it is connected to. If
/// you need to toggle shellies on multiple brokers you need multiple instanced of this actuator.
struct Shelly {
    signal: Option<StatusSignal>,
    waker: Option<Waker>,
    name: String,
    client: mqtt::AsyncClient,
}

impl Shelly {
    // Can't use Error, it's not Send. fabinfra/fabaccess/bffh#7
    pub async fn new(config: Settings) -> Self {
        let client = mqtt::AsyncClient::new(config.shelly.unwrap().mqtt_url).unwrap();

        client.connect(mqtt::ConnectOptions::new()).await.unwrap();

        let name = "test".to_string();
        let signal: Option<StatusSignal> = None;
        let waker = None;

        Shelly { signal, waker, name, client }
    }
}


impl Actuator for Shelly {
    fn subscribe(&mut self, signal: StatusSignal) {
        self.signal.replace(signal);
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }
}

impl Stream for Shelly {
    type Item = future::BoxFuture<'static, ()>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let unpin = Pin::into_inner(self);
        if let Some(ref mut s) = unpin.signal {
            if let Some(status) = ready!(Signal::poll_change(Pin::new(s), cx)) {
                let topic = format!("shellies/{}/relay/0/command", unpin.name);
                let pl = match status {
                    Status::Free | Status::Blocked => "off",
                    Status::Occupied => "on",
                };
                let msg = mqtt::Message::new(topic, pl, 0);
                let f = unpin.client.publish(msg).map(|_| ());

                return Poll::Ready(Some(Box::pin(f)));
            }
        } else {
            unpin.waker.replace(cx.waker().clone());
        }

        Poll::Pending
    }
}
