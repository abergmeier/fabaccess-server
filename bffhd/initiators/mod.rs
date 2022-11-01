use crate::initiators::dummy::Dummy;
use crate::initiators::process::Process;
use crate::resources::modules::fabaccess::Status;
use crate::session::SessionHandle;
use crate::{
    AuthenticationHandle, Config, MachineState, Resource, ResourcesHandle, SessionManager,
};
use async_compat::CompatExt;
use executor::prelude::Executor;
use futures_util::ready;
use miette::IntoDiagnostic;
use rumqttc::ConnectReturnCode::Success;
use rumqttc::{AsyncClient, ConnectionError, Event, Incoming, MqttOptions};
use std::collections::HashMap;
use std::fmt::Display;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tracing::Span;
use url::Url;

mod dummy;
mod process;

pub trait Initiator: Future<Output = ()> {
    fn new(params: &HashMap<String, String>, callbacks: InitiatorCallbacks) -> miette::Result<Self>
    where
        Self: Sized;
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<()> {
        <Self as Future>::poll(self, cx)
    }
}

#[derive(Clone)]
pub struct InitiatorCallbacks {
    span: Span,
    resource: Resource,
    sessions: SessionManager,
}
impl InitiatorCallbacks {
    pub fn new(span: Span, resource: Resource, sessions: SessionManager) -> Self {
        Self {
            span,
            resource,
            sessions,
        }
    }

    pub async fn try_update(&mut self, session: SessionHandle, status: Status) {
        self.resource.try_update(session, status).await
    }

    pub fn set_status(&mut self, status: Status) {
        self.resource.set_status(status)
    }

    pub fn open_session(&self, uid: &str) -> Option<SessionHandle> {
        self.sessions.try_open(&self.span, uid)
    }
}

pub struct InitiatorDriver {
    span: Span,
    name: String,
    initiator: Box<dyn Initiator + Unpin + Send>,
}

impl InitiatorDriver {
    pub fn new<I>(
        span: Span,
        name: String,
        params: &HashMap<String, String>,
        resource: Resource,
        sessions: SessionManager,
    ) -> miette::Result<Self>
    where
        I: 'static + Initiator + Unpin + Send,
    {
        let callbacks = InitiatorCallbacks::new(span.clone(), resource, sessions);
        let initiator = Box::new(I::new(params, callbacks)?);
        Ok(Self {
            span,
            name,
            initiator,
        })
    }
}

impl Future for InitiatorDriver {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let _guard = tracing::info_span!("initiator poll", initiator=%self.name);
        tracing::trace!(initiator=%self.name, "polling initiator");

        ready!(Pin::new(&mut self.initiator).poll(cx));

        tracing::warn!(initiator=%self.name, "initiator module ran to completion!");

        Poll::Ready(())
    }
}

pub fn load(
    executor: Executor,
    config: &Config,
    resources: ResourcesHandle,
    sessions: SessionManager,
    authentication: AuthenticationHandle,
) -> miette::Result<()> {
    let span = tracing::info_span!("loading initiators");
    let _guard = span.enter();

    let mut initiator_map: HashMap<String, Resource> = config
        .init_connections
        .iter()
        .filter_map(|(k, v)| {
            if let Some(resource) = resources.get_by_id(v) {
                Some((k.clone(), resource.clone()))
            } else {
                tracing::error!(initiator=%k, machine=%v,
                    "Machine configured for initiator not found!");
                None
            }
        })
        .collect();

    for (name, cfg) in config.initiators.iter() {
        if let Some(resource) = initiator_map.remove(name) {
            if let Some(driver) = load_single(name, &cfg.module, &cfg.params, resource, &sessions) {
                tracing::debug!(module_name=%cfg.module, %name, "starting initiator task");
                executor.spawn(driver);
            } else {
                tracing::error!(module_name=%cfg.module, %name, "Initiator module could not be configured");
            }
        } else {
            tracing::warn!(actor=%name, ?config, "Initiator has no machine configured. Skipping!");
        }
    }

    Ok(())
}

fn load_single(
    name: &String,
    module_name: &String,
    params: &HashMap<String, String>,
    resource: Resource,
    sessions: &SessionManager,
) -> Option<InitiatorDriver> {
    let span = tracing::info_span!(
        "initiator",
        name = %name,
        module = %module_name,
    );
    tracing::info!(%name, %module_name, ?params, "Loading initiator");
    let o = match module_name.as_ref() {
        "Dummy" => Some(InitiatorDriver::new::<Dummy>(
            span,
            name.clone(),
            params,
            resource,
            sessions.clone(),
        )),
        "Process" => Some(InitiatorDriver::new::<Process>(
            span,
            name.clone(),
            params,
            resource,
            sessions.clone(),
        )),
        _ => None,
    };

    o.transpose().unwrap_or_else(|error| {
        tracing::error!(%error, "failed to configure initiator");
        None
    })
}
