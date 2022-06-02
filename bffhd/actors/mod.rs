use crate::actors::shelly::Shelly;
use crate::resources::state::State;
use crate::{Config, ResourcesHandle};
use async_compat::CompatExt;
use executor::pool::Executor;
use futures_signals::signal::Signal;
use futures_util::future::BoxFuture;
use rumqttc::{AsyncClient, ConnectionError, Event, Incoming, MqttOptions};

use std::collections::HashMap;
use std::future::Future;

use std::pin::Pin;

use miette::IntoDiagnostic;
use std::task::{Context, Poll};
use std::time::Duration;

use once_cell::sync::Lazy;
use rumqttc::ConnectReturnCode::Success;

use crate::actors::dummy::Dummy;
use crate::actors::process::Process;
use crate::db::ArchivedValue;
use rustls::RootCertStore;
use url::Url;

mod dummy;
mod process;
mod shelly;

pub trait Actor {
    fn apply(&mut self, state: ArchivedValue<State>) -> BoxFuture<'static, ()>;
}

pub struct ActorDriver<S: 'static> {
    signal: S,

    actor: Box<dyn Actor + Send + Sync>,
    future: Option<BoxFuture<'static, ()>>,
}

impl<S: Signal<Item = ArchivedValue<State>>> ActorDriver<S> {
    pub fn new(signal: S, actor: Box<dyn Actor + Send + Sync>) -> Self {
        Self {
            signal,
            actor,
            future: None,
        }
    }
}

impl<S> Future for ActorDriver<S>
where
    S: Signal<Item = ArchivedValue<State>> + Unpin + Send,
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        // Work until there is no more work to do.
        loop {
            // Poll the `apply` future. And ensure it's completed before the next one is started
            match self
                .future
                .as_mut()
                .map(|future| Future::poll(Pin::new(future), cx))
            {
                // Skip and poll for a new future to do
                None => {}

                // This apply future is done, get a new one
                Some(Poll::Ready(_)) => self.future = None,

                // This future would block so we return to continue work another time
                Some(Poll::Pending) => return Poll::Pending,
            }

            // Poll the signal and apply any change that happen to the inner Actuator
            match Pin::new(&mut self.signal).poll_change(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => return Poll::Ready(()),
                Poll::Ready(Some(state)) => {
                    // This future MUST be polled before we exit from the Actor::poll because if we
                    // do not do that it will not register the dependency and thus NOT BE POLLED.
                    let f = self.actor.apply(state);
                    self.future.replace(f);
                }
            }
        }
    }
}

static ROOT_CERTS: Lazy<RootCertStore> = Lazy::new(|| {
    let span = tracing::info_span!("loading system certificates");
    let _guard = span.enter();
    let mut store = RootCertStore::empty();
    match rustls_native_certs::load_native_certs() {
        Ok(certs) => {
            let certs: Vec<Vec<u8>> = certs.into_iter().map(|c| c.0).collect();
            let (loaded, ignored) = store.add_parsable_certificates(&certs[..]);
            if ignored != 0 {
                tracing::info!(loaded, ignored, "certificates loaded, some ignored");
            } else {
                tracing::info!(loaded, "certificates loaded");
            }
        }
        Err(error) => {
            tracing::error!(%error, "failed to load system certificates");
        }
    }
    store
});

pub fn load(executor: Executor, config: &Config, resources: ResourcesHandle) -> miette::Result<()> {
    let span = tracing::info_span!("loading actors");
    let _guard = span;

    let mqtt_url = Url::parse(config.mqtt_url.as_str()).into_diagnostic()?;
    let (transport, default_port) = match mqtt_url.scheme() {
        "mqtts" | "ssl" => (
            rumqttc::Transport::tls_with_config(
                rumqttc::ClientConfig::builder()
                    .with_safe_defaults()
                    .with_root_certificates(ROOT_CERTS.clone())
                    .with_no_client_auth()
                    .into(),
            ),
            8883,
        ),

        "mqtt" | "tcp" => (rumqttc::Transport::tcp(), 1883),

        scheme => {
            tracing::error!(%scheme, "MQTT url uses invalid scheme");
            miette::bail!("invalid config");
        }
    };
    let host = mqtt_url.host_str().ok_or_else(|| {
        tracing::error!("MQTT url must contain a hostname");
        miette::miette!("invalid config")
    })?;
    let port = mqtt_url.port().unwrap_or(default_port);

    let mut mqttoptions = MqttOptions::new("bffh", host, port);

    mqttoptions
        .set_transport(transport)
        .set_keep_alive(Duration::from_secs(20));

    if !mqtt_url.username().is_empty() {
        mqttoptions.set_credentials(mqtt_url.username(), mqtt_url.password().unwrap_or_default());
    }

    let (mqtt, mut eventloop) = AsyncClient::new(mqttoptions, 256);
    let mut eventloop = executor.run(
        async move {
            match eventloop.poll().await {
                Ok(Event::Incoming(Incoming::Connect(_connect))) => {}
                Ok(Event::Incoming(Incoming::ConnAck(connack))) => {
                    if connack.code == Success {
                        tracing::debug!(?connack, "MQTT connection established");
                    } else {
                        tracing::error!(?connack, "MQTT connect failed");
                    }
                }
                Ok(event) => {
                    tracing::warn!(?event, "Got unexpected mqtt event");
                }
                Err(error) => {
                    tracing::error!(?error, "MQTT connection failed");
                    miette::bail!("mqtt connection failed")
                }
            }

            Ok(eventloop)
        }
        .compat(),
    )?;

    executor.spawn(
        async move {
            let mut fault = false;
            loop {
                match eventloop.poll().compat().await {
                    Ok(_) => {
                        fault = false;
                        // TODO: Handle incoming MQTT messages
                    }
                    Err(ConnectionError::Cancel)
                    | Err(ConnectionError::StreamDone)
                    | Err(ConnectionError::RequestsDone) => {
                        // Normal exit
                        tracing::info!("MQTT request queue closed, stopping client.");
                        return;
                    }
                    Err(ConnectionError::Timeout(_)) => {
                        tracing::error!("MQTT operation timed out!");
                        tracing::warn!(
                            "MQTT client will continue, but messages may have been lost."
                        )
                        // Timeout does not close the client
                    }
                    Err(ConnectionError::Io(error)) if fault => {
                        tracing::error!(?error, "MQTT recurring IO error, closing client");
                        // Repeating IO errors close client. Any Ok() in between resets fault to false.
                        return;
                    }
                    Err(ConnectionError::Io(error)) => {
                        fault = true;
                        tracing::error!(?error, "MQTT encountered IO error");
                        // *First* IO error does not close the client.
                    }
                    Err(error) => {
                        tracing::error!(?error, "MQTT client encountered unhandled error");
                        return;
                    }
                }
            }
        }
        .compat(),
    );

    let mut actor_map: HashMap<String, _> = config
        .actor_connections
        .iter()
        .filter_map(|(k, v)| {
            if let Some(resource) = resources.get_by_id(v) {
                Some((k.clone(), resource.get_signal()))
            } else {
                tracing::error!(actor=%k, machine=%v, "Machine configured for actor not found!");
                None
            }
        })
        .collect();

    for (name, cfg) in config.actors.iter() {
        if let Some(sig) = actor_map.remove(name) {
            if let Some(actor) = load_single(name, &cfg.module, &cfg.params, mqtt.clone()) {
                let driver = ActorDriver::new(sig, actor);
                tracing::debug!(module_name=%cfg.module, %name, "starting actor task");
                executor.spawn(driver);
            } else {
                tracing::error!(module_name=%cfg.module, %name, "Actor module type not found");
            }
        } else {
            tracing::warn!(actor=%name, ?config, "Actor has no machine configured. Skipping!");
        }
    }

    Ok(())
}

fn load_single(
    name: &String,
    module_name: &String,
    params: &HashMap<String, String>,
    client: AsyncClient,
) -> Option<Box<dyn Actor + Sync + Send>> {
    tracing::info!(%name, %module_name, ?params, "Loading actor");
    match module_name.as_ref() {
        "Dummy" => Some(Box::new(Dummy::new(name.clone(), params.clone()))),
        "Process" => Process::new(name.clone(), params).map(|a| a.into_boxed_actuator()),
        "Shelly" => Some(Box::new(Shelly::new(name.clone(), client, params))),
        _ => None,
    }
}
