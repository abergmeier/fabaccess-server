use slog::Logger;

use crate::config::Settings;
use crate::registries::{Registries, Actuator, ActBox, StatusSignal};
use crate::error::Result;
use crate::machine::Status;

use std::pin::Pin;
use futures::prelude::*;
use futures::channel::mpsc;
use futures::ready;
use futures::task::{Poll, Context, Waker, Spawn, FutureObj};
use futures::StreamExt;
use futures_signals::signal::Signal;

use paho_mqtt as mqtt;

// TODO: Late config parsing. Right now the config is validated at the very startup in its
// entirety. This works reasonably enough for this static modules here but if we do dynamic loading
// via dlopen(), lua API, python API etc it will not.
pub async fn run<S: Spawn>(log: Logger, config: Settings, registries: Registries, spawner: S) {
    let (tx, rx) = mpsc::channel(1);
    let mut shelly = Shelly::new(log, config, rx).await;

    let r = registries.actuators.register("shelly".to_string(), tx).await;

    let f = shelly.for_each(|f| f);
    spawner.spawn_obj(FutureObj::from(Box::pin(f)));

}

/// An actuator for all Shellies connected listening on one MQTT broker
///
/// This actuator can power toggle an arbitrariy named shelly on the broker it is connected to. If
/// you need to toggle shellies on multiple brokers you need multiple instanced of this actuator.
struct Shelly {
    log: Logger,
    sigchan: mpsc::Receiver<StatusSignal>,
    signal: Option<StatusSignal>,
    waker: Option<Waker>,
    name: String,
    client: mqtt::AsyncClient,
}

impl Shelly {
    // Can't use Error, it's not Send. fabinfra/fabaccess/bffh#7
    pub async fn new(log: Logger, config: Settings, sigchan: mpsc::Receiver<StatusSignal>) -> Self {
        let client = mqtt::AsyncClient::new(config.shelly.unwrap().mqtt_url).unwrap();

        let o = client.connect(mqtt::ConnectOptions::new()).await.unwrap();
        println!("{:?}", o);

        let name = "test".to_string();
        let signal: Option<StatusSignal> = None;
        let waker = None;

        Shelly { log, sigchan, signal, waker, name, client }
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

        info!(unpin.log, "tick {}", unpin.signal.is_some());

        if let Poll::Ready(v) = Stream::poll_next(Pin::new(&mut unpin.sigchan), cx) {
            if let Some(s) = v {
                // We have received a new signal to use
                unpin.signal.replace(s);
                // We use `if let` instead of .and_then because we want the waker to be dropped
                // afterwards. It's only there to ensure the future is called when a signal is
                // installed the first time
                // TODO probably don't need that here because we're polling it either way directly
                // afterwards, eh?
                if let Some(waker) = unpin.waker.take() {
                    waker.wake();
                }
            } else {
                info!(unpin.log, "bye");
                // This means that the sending end was dropped, so we shut down
                unpin.signal.take();
                unpin.waker.take();
                return Poll::Ready(None);
            }
        }

        if let Some(ref mut s) = unpin.signal {
            if let Some(status) = ready!(Signal::poll_change(Pin::new(s), cx)) {
                info!(unpin.log, "Machine Status changed: {:?}", status);
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
            info!(unpin.log, "I ain't got no signal son");
            unpin.waker.replace(cx.waker().clone());
        }

        Poll::Pending
    }
}
