#![warn(unused_imports, unused_import_braces)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![warn(missing_crate_level_docs)]

//! Diflouroborane
//!
//! This is the capnp component of the FabAccess project.
//! The entry point of bffhd can be found in [bin/bffhd/main.rs](../bin/bffhd/main.rs)

pub mod config;

/// Internal Databases build on top of LMDB, a mmap()'ed B-tree DB optimized for reads
pub mod db;

/// Shared error type
pub mod error;

pub mod users;
pub mod authentication;
pub mod authorization;

/// Resources
pub mod resources;

pub mod actors;

pub mod initiators;

pub mod sensors;

pub mod capnp;

pub mod utils;

mod tls;
mod keylog;
mod logging;
mod audit;
mod session;

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use anyhow::Context;
use futures_rustls::TlsAcceptor;
use futures_util::StreamExt;
use once_cell::sync::OnceCell;
use rustls::{Certificate, KeyLogFile, PrivateKey, ServerConfig};
use rustls::server::NoClientAuth;
use signal_hook::consts::signal::*;
use executor::pool::Executor;
use crate::authentication::AuthenticationHandle;
use crate::capnp::APIServer;
use crate::config::{Config, TlsListen};
use crate::resources::modules::fabaccess::MachineState;
use crate::resources::Resource;
use crate::resources::search::ResourcesHandle;
use crate::resources::state::db::StateDB;
use crate::session::SessionManager;
use crate::tls::TlsConfig;
use crate::users::db::UserDB;
use crate::users::Users;

pub const RELEASE_STRING: &'static str = env!("BFFHD_RELEASE_STRING");

pub struct Diflouroborane {
    executor: Executor<'static>,
}

pub static RESOURCES: OnceCell<ResourcesHandle> = OnceCell::new();

impl Diflouroborane {
    pub fn new() -> Self {
        let executor = Executor::new();

        Self { executor }
    }

    fn log_version_number(&self) {
        tracing::info!(version=RELEASE_STRING, "Starting");
    }

    pub fn init_logging(config: &Config) {
        logging::init(&config);
    }

    pub fn setup(&mut self, config: &Config) -> anyhow::Result<()> {
        Self::init_logging(config);

        let span = tracing::info_span!("setup");
        let _guard = span.enter();

        self.log_version_number();

        let mut signals = signal_hook_async_std::Signals::new(&[
            SIGINT,
            SIGQUIT,
            SIGTERM,
        ]).context("Failed to construct signal handler")?;

        let env = StateDB::open_env(&config.db_path)?;
        let statedb = Arc::new(StateDB::create_with_env(env.clone())
            .context("Failed to open state DB file")?);

        let userdb = Users::new(env.clone()).context("Failed to open users DB file")?;

        let resources = ResourcesHandle::new(config.machines.iter().map(|(id, desc)| {
            Resource::new(Arc::new(resources::Inner::new(id.to_string(), statedb.clone(), desc.clone())))
        }));
        RESOURCES.set(resources.clone());

        actors::load(self.executor.clone(), &config, resources.clone())?;


        let tlsconfig = TlsConfig::new(config.tlskeylog.as_ref(), !config.is_quiet())?;
        let acceptor = tlsconfig.make_tls_acceptor(&config.tlsconfig)?;

        let sessionmanager = SessionManager::new(userdb.clone());
        let authentication = AuthenticationHandle::new(userdb.clone());

        let mut apiserver = self.executor.run(APIServer::bind(self.executor.clone(), &config.listens, acceptor, sessionmanager, authentication))?;

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

