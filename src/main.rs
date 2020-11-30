#[macro_use]
extern crate slog;

#[macro_use]
extern crate capnp_rpc;

#[macro_use]
extern crate async_trait;

mod modules;
mod log;
mod api;
mod config;
mod error;
mod connection;
mod registries;
mod schema;
mod db;
mod machine;
mod builtin;
mod server;

use clap::{App, Arg};

use futures::prelude::*;
use futures::executor::{LocalPool, ThreadPool};
use futures::compat::Stream01CompatExt;
use futures::join;
use futures::task::LocalSpawn;

use std::io;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;

use std::sync::Arc;

use lmdb::Transaction;
use smol::net::TcpListener;

use error::Error;

use registries::Registries;

// Returning a `Result` from `main` allows us to use the `?` shorthand.
// In the case of an Err it will be printed using `fmt::Debug`
fn main() -> Result<(), Error> {
    use clap::{crate_version, crate_description, crate_name};

    // Argument parsing
    // values for the name, description and version are pulled from `Cargo.toml`.
    let matches = App::new(crate_name!())
        .about(crate_description!())
        .version(crate_version!())
        .arg(Arg::with_name("config")
            .help("Path to the config file to use")
            .long("config")
            .short("c")
            .takes_value(true)
        )
        .arg(Arg::with_name("print default")
            .help("Print a default config to stdout instead of running")
            .long("print-default")
        )
        .arg(Arg::with_name("dump")
            .help("Dump all databases into the given directory")
            .long("dump")
            .conflicts_with("load")
            .takes_value(true)
        )
        .arg(Arg::with_name("load")
            .help("Load databases from the given directory")
            .long("load")
            .conflicts_with("dump")
            .takes_value(true)
        )
        .get_matches();

    // Check for the --print-default option first because we don't need to do anything else in that
    // case.
    if matches.is_present("print default") {
        let config = config::Settings::default();
        let encoded = toml::to_vec(&config)?;

        // Direct writing to fd 1 is faster but also prevents any print-formatting that could
        // invalidate the generated TOML
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        handle.write_all(&encoded)?;

        // Early return to exit.
        return Ok(())
    } else if matches.is_present("dump") {
    } else if matches.is_present("load") {
    } else {
    }


    // If no `config` option is given use a preset default.
    let configpath = matches.value_of("config").unwrap_or("/etc/bffh/config.toml");
    let config = config::read(&PathBuf::from_str(configpath).unwrap())?;

    // Initialize the logging subsystem first to be able to better document the progress from now
    // on.
    // TODO: Now would be a really good time to close stdin/out and move logging to syslog
    // Log is in an Arc so we can do very cheap clones in closures.
    let log = Arc::new(log::init(&config));
    info!(log, "Starting");

    let db = db::Databases::new(&log, &config)?;

    server::serve_api_connections(log, config, db)
}

/// The result of one iteration of the core loop
pub enum LoopResult {
    /// Everything was fine, keep going
    Continue,
    /// Something happened that means we should shut down
    Stop,
    /// The Server is currently overloaded
    Overloaded,
}
