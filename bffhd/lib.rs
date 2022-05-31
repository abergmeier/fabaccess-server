#![warn(unused_imports, unused_import_braces)]
//#![warn(missing_debug_implementations)]
//#![warn(missing_docs)]
//#![warn(missing_crate_level_docs)]

//! Diflouroborane
//!
//! This is the capnp component of the FabAccess project.
//! The entry point of bffhd can be found in [bin/bffhd/main.rs](../bin/bffhd/main.rs)

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

use anyhow::Context;

use futures_util::StreamExt;
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
use signal_hook::consts::signal::*;

pub struct Diflouroborane {
    config: Config,
    executor: Executor<'static>,
    pub statedb: StateDB,
    pub users: Users,
    pub roles: Roles,
    pub resources: ResourcesHandle,
}

pub static RESOURCES: OnceCell<ResourcesHandle> = OnceCell::new();

impl Diflouroborane {
    pub fn new(config: Config) -> anyhow::Result<Self> {
        logging::init(&config.logging);
        tracing::info!(version = env::VERSION, "Starting BFFH");

        let span = tracing::info_span!("setup");
        let _guard = span.enter();

        let executor = Executor::new();

        let env = StateDB::open_env(&config.db_path)
            .context("Failed to create state DB env. Does the parent directory for `db_path` exist?")?;
        let statedb =
            StateDB::create_with_env(env.clone()).context("Failed to open state DB file")?;

        let users = Users::new(env.clone()).context("Failed to open users DB file")?;
        let roles = Roles::new(config.roles.clone());

        let _audit_log = AuditLog::new(&config).context("Failed to initialize audit log")?;

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
        })
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        let mut signals = signal_hook_async_std::Signals::new(&[SIGINT, SIGQUIT, SIGTERM])
            .context("Failed to construct signal handler")?;

        actors::load(self.executor.clone(), &self.config, self.resources.clone())?;

        let tlsconfig = TlsConfig::new(self.config.tlskeylog.as_ref(), !self.config.is_quiet())?;
        let acceptor = tlsconfig.make_tls_acceptor(&self.config.tlsconfig)?;

        let sessionmanager = SessionManager::new(self.users.clone(), self.roles.clone());
        let authentication = AuthenticationHandle::new(self.users.clone());

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
