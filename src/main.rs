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
mod network;
mod actor;
mod initiator;

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

use smol::Executor;

use error::Error;

use slog::Logger;

use registries::Registries;

fn main() {
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
        let encoded = toml::to_vec(&config).unwrap();

        // Direct writing to fd 1 is faster but also prevents any print-formatting that could
        // invalidate the generated TOML
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        handle.write_all(&encoded).unwrap();

        // Early return to exit.
        return Ok(());
    }

    let retval;

    // Scope to drop everything before exiting.
    {
        // Initialize the logging subsystem first to be able to better document the progress from now
        // on.
        // TODO: Now would be a really good time to close stdin/out and move logging to syslog
        // Log is in an Arc so we can do very cheap clones in closures.
        let log = Arc::new(log::init());
        info!(log, "Starting");

        match maybe(matches, log.clone()) {
            Ok(_) => retval = 0,
            Err(e) => {
                error!(log, "{}", e);
                retval = -1;
            }
        }
    }

    std::process::exit(retval);
}

// Returning a `Result` from `main` allows us to use the `?` shorthand.
// In the case of an Err it will be printed using `fmt::Debug`
fn maybe(matches: clap::ArgMatches, log: Arc<Logger>) -> Result<(), Error> {
    // If no `config` option is given use a preset default.
    let configpath = matches.value_of("config").unwrap_or("/etc/bffh/config.toml");
    let config = config::read(&PathBuf::from_str(configpath).unwrap())?;


    if matches.is_present("dump") {
        error!(log, "Dumping is currently not implemented");
        Ok(())
    } else if matches.is_present("load") {
        error!(log, "Loading is currently not implemented");
        Ok(())
    } else {
        let db = db::Databases::new(&log, &config)?;

        // TODO: Spawn api connections on their own (non-main) thread, use the main thread to
        // handle signals (a cli if stdin is not closed?) and make it stop and clean up all threads
        // when bffh should exit
        let machines = machine::load(&config)?;
        let actors = actor::load(&config)?;
        let initiators = load_initiators(&config)?;

        // TODO restore connections between initiators, machines, actors

        let ex = Arc::new(Executor::new());

        for i in initiators.into_iter() {
            ex.spawn(i.run());
        }

        for a in actors.into_iter() {
            ex.spawn(a.run());
        }

        let (signal, shutdown) = futures::channel::oneshot::channel();
        for i in 0..4 {
            std::thread::spawn(|| smol::block_on(ex.run(shutdown.recv())));
        }

        server::serve_api_connections(log.clone(), config, db)
        // Signal is dropped here, stopping all executor threads as well.
    }
}
