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

use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;
use anyhow::Context;
use futures_rustls::TlsAcceptor;
use rustls::{Certificate, KeyLogFile, PrivateKey, ServerConfig};
use rustls::server::NoClientAuth;
use signal_hook::consts::signal::*;
use executor::pool::Executor;
use crate::capnp::APIServer;
use crate::config::{Config, TlsListen};
use crate::tls::TlsConfig;

pub struct Diflouroborane {
    executor: Executor<'static>,
}

impl Diflouroborane {
    pub fn new() -> Self {
        let executor = Executor::new();

        Self { executor }
    }

    fn log_version_number(&self) {
        const RELEASE_STRING: &'static str = env!("BFFHD_RELEASE_STRING");
        tracing::info!(version=RELEASE_STRING, "Starting");
    }

    pub fn setup(&mut self, config: &Config) -> anyhow::Result<()> {
        logging::init(&config);

        let span = tracing::info_span!("setup");
        let _guard = span.enter();

        self.log_version_number();

        let signals = signal_hook_async_std::Signals::new(&[
            SIGINT,
            SIGQUIT,
            SIGTERM,
        ]).context("Failed to construct signal handler")?;
        tracing::debug!("Set up signal handler");

        let tlsconfig = TlsConfig::new(config.tlskeylog.as_ref(), !config.is_quiet())?;
        let acceptor = tlsconfig.make_tls_acceptor(&config.tlsconfig)?;

        APIServer::bind(self.executor.clone(), &config.listens, acceptor);

        Ok(())
    }
}