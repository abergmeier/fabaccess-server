#![warn(unused_imports, unused_import_braces)]
//#![warn(missing_debug_implementations)]
//#![warn(missing_docs)]
//#![warn(missing_crate_level_docs)]

//! Diflouroborane
//!
//! This is the capnp component of the FabAccess project.
//! The entry point of bffhd can be found in [bin/bffhd/main.rs](../bin/bffhd/main.rs)

use miette::Diagnostic;
use std::io;
use thiserror::Error;

pub mod config;

/// Internal Databases build on top of LMDB, a mmap()'ed B-tree DB optimized for reads
pub mod db;

/// Shared error type
pub mod error;

pub mod authentication;
pub mod authorization;
pub mod users;

/// Resources
pub mod resources;

pub mod actors;
pub mod initiators;

pub mod sensors;

pub mod capnp;

pub mod utils;

// Store build information in the `env` module.
shadow_rs::shadow!(env);

mod audit;
mod keylog;
mod logging;
mod session;
mod tls;

use std::sync::Arc;

use futures_util::{FutureExt, StreamExt};
use miette::{Context, IntoDiagnostic, Report};
use once_cell::sync::OnceCell;

use crate::audit::AuditLog;
use crate::authentication::AuthenticationHandle;
use crate::authorization::roles::Roles;
use crate::capnp::APIServer;
use crate::config::Config;
use crate::resources::modules::fabaccess::MachineState;
use crate::resources::search::ResourcesHandle;
use crate::resources::state::db::StateDB;
use crate::resources::Resource;
use crate::session::SessionManager;
use crate::tls::TlsConfig;
use crate::users::db::UserDB;
use crate::users::Users;
use executor::pool::Executor;
use lightproc::recoverable_handle::RecoverableHandle;
use signal_hook::consts::signal::*;
use tracing::Span;

pub struct Diflouroborane {
    config: Config,
    executor: Executor<'static>,
    pub statedb: StateDB,
    pub users: Users,
    pub roles: Roles,
    pub resources: ResourcesHandle,
    span: Span,
}

pub static RESOURCES: OnceCell<ResourcesHandle> = OnceCell::new();

struct SignalHandlerErr;
impl error::Description for SignalHandlerErr {
    const CODE: &'static str = "signals::new";
}

#[derive(Debug, Error, Diagnostic)]
pub enum BFFHError {
    #[error("DB operation failed")]
    DBError(
        #[from]
        #[source]
        db::Error,
    ),
    #[error("failed to initialize global user store")]
    UsersError(
        #[from]
        #[source]
        users::Error,
    ),
    #[error("failed to initialize state database")]
    StateDBError(
        #[from]
        #[source]
        resources::state::db::StateDBError,
    ),
    #[error("audit log failed")]
    AuditLogError(
        #[from]
        #[source]
        audit::Error,
    ),
    #[error("Failed to initialize signal handler")]
    SignalsError(#[source] std::io::Error),
    #[error("error in actor subsystem")]
    ActorError(
        #[from]
        #[source]
        actors::ActorError,
    ),
    #[error("failed to initialize TLS config")]
    TlsSetup(
        #[from]
        #[source]
        tls::Error,
    ),
    #[error("API handler failed")]
    ApiError(
        #[from]
        #[source]
        capnp::Error,
    ),
}

impl Diflouroborane {
    pub fn setup() {}

    pub fn new(config: Config) -> Result<Self, BFFHError> {
        let mut server = logging::init(&config.logging);
        let span = tracing::info_span!(
            target: "bffh",
            "bffh"
        );
        let span2 = span.clone();
        let _guard = span2.enter();
        tracing::info!(version = env::VERSION, "Starting BFFH");

        let executor = Executor::new();

        if let Some(aggregator) = server.aggregator.take() {
            executor.spawn(aggregator.run());
        }
        tracing::info!("Server is being spawned");
        let handle = executor.spawn(server.serve());
        executor.spawn(handle.map(|result| match result {
            Some(Ok(())) => {
                tracing::info!("console server finished without error");
            }
            Some(Err(error)) => {
                tracing::info!(%error, "console server finished with error");
            }
            None => {
                tracing::info!("console server finished with panic");
            }
        }));

        let env = StateDB::open_env(&config.db_path)?;

        let statedb = StateDB::create_with_env(env.clone())?;

        let users = Users::new(env.clone())?;
        let roles = Roles::new(config.roles.clone());

        let _audit_log = AuditLog::new(&config)?;

        let resources = ResourcesHandle::new(config.machines.iter().map(|(id, desc)| {
            Resource::new(Arc::new(resources::Inner::new(
                id.to_string(),
                statedb.clone(),
                desc.clone(),
            )))
        }));
        RESOURCES.set(resources.clone());

        Ok(Self {
            config,
            executor,
            statedb,
            users,
            roles,
            resources,
            span,
        })
    }

    pub fn run(&mut self) -> Result<(), BFFHError> {
        let _guard = self.span.enter();
        let mut signals = signal_hook_async_std::Signals::new(&[SIGINT, SIGQUIT, SIGTERM])
            .map_err(BFFHError::SignalsError)?;

        let sessionmanager = SessionManager::new(self.users.clone(), self.roles.clone());
        let authentication = AuthenticationHandle::new(self.users.clone());

        initiators::load(
            self.executor.clone(),
            &self.config,
            self.resources.clone(),
            sessionmanager.clone(),
            authentication.clone(),
        );
        actors::load(self.executor.clone(), &self.config, self.resources.clone())?;

        let tlsconfig = TlsConfig::new(self.config.tlskeylog.as_ref(), !self.config.is_quiet())?;
        let acceptor = tlsconfig.make_tls_acceptor(&self.config.tlsconfig)?;

        let apiserver = self.executor.run(APIServer::bind(
            self.executor.clone(),
            &self.config.listens,
            acceptor,
            sessionmanager,
            authentication,
        ))?;

        let (mut tx, rx) = async_oneshot::oneshot();

        self.executor.spawn(apiserver.handle_until(rx));

        let f = async {
            let mut sig = None;
            while {
                sig = signals.next().await;
                sig.is_none()
            } {}
            tracing::info!(signal = %sig.unwrap(), "Received signal");
            tx.send(());
        };

        self.executor.run(f);
        Ok(())
    }
}

struct ShutdownHandler {
    tasks: Vec<RecoverableHandle<()>>,
}
impl ShutdownHandler {
    pub fn new(tasks: Vec<RecoverableHandle<()>>) -> Self {
        Self { tasks }
    }

    pub fn shutdown(&mut self) {
        for handle in self.tasks.drain(..) {
            handle.cancel()
        }
    }
}
