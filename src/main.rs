// FIXME: No.
#![allow(dead_code)]

#[macro_use]
extern crate slog;

#[macro_use]
extern crate capnp_rpc;

extern crate async_trait;

mod modules;
mod log;
mod api;
mod config;
mod error;
mod connection;
mod schema;
mod db;
mod machine;
mod builtin;
mod server;
mod network;
mod actor;
mod initiator;
mod space;

use clap::{App, Arg};

use std::io;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;

use std::sync::Arc;

use smol::Executor;

use error::Error;

use slog::Logger;

use paho_mqtt::AsyncClient;
use crate::config::{ActorConn, Config, InitiatorConn};

const RELEASE: &'static str = env!("BFFHD_RELEASE_STRING");
const VERSION: &'static str = env!("CARGO_PKG_VERSION");
const GITREV: &'static str = env!("CARGO_PKG_VERSION_GITREV");

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
        .arg(Arg::with_name("check config")
            .help("Check config for validity")
            .long("check")
        )
        .arg(Arg::with_name("dump")
            .help("Dump all databases into the given directory")
            .long("dump")
            .conflicts_with("load")
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
        let config = config::Config::default();
        let encoded = serde_dhall::serialize(&config).to_string().unwrap();

        // Direct writing to fd 1 is faster but also prevents any print-formatting that could
        // invalidate the generated TOML
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        handle.write_all(&encoded.as_bytes()).unwrap();

        // Early return to exit.
        return;
    } else if matches.is_present("check config") {
        let configpath = matches.value_of("config").unwrap_or("/etc/diflouroborane.dhall");
        match config::read(&PathBuf::from_str(configpath).unwrap()) {
            Ok(cfg) => {
                //TODO: print a normalized version of the supplied config
                println!("config is valid");
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(-1);
            }
        }
    }

    let retval;

    // Scope to drop everything before exiting.
    {
        // Initialize the logging subsystem first to be able to better document the progress from now
        // on.
        // TODO: Now would be a really good time to close stdin/out and move logging to syslog
        // Log is in an Arc so we can do very cheap clones in closures.
        let (log, guard) = log::init();
        let log = Arc::new(log);
        info!(log, "Starting");

        match maybe(matches, log.clone()) {
            Ok(_) => retval = 0,
            Err(e) => {
                error!(log, "{}", e);
                retval = -1;
            }
        }
        drop(guard);
    }
}

// Returning a `Result` from `main` allows us to use the `?` shorthand.
// In the case of an Err it will be printed using `fmt::Debug`
fn maybe(matches: clap::ArgMatches, log: Arc<Logger>) -> Result<(), Error> {
    // If no `config` option is given use a preset default.
    let configpath = matches.value_of("config").unwrap_or("/etc/diflouroborane.dhall");
    let config = config::read(&PathBuf::from_str(configpath).unwrap())?;
    debug!(log, "Loaded Config: {:?}", config);

    if matches.is_present("dump") {
        let db = db::Databases::new(&log, &config)?;
        let v = db.access.dump_roles().unwrap();
        for (id, role) in v.iter() {
            info!(log, "Role {}:\n{}", id, role);
        }

        let v = db.userdb.list_users()?;
        for user in v.iter() {
            info!(log, "User {}:\n{:?}", user.id, user.data);
        }
        Ok(())
    } else if matches.is_present("load") {
        let db = db::Databases::new(&log, &config)?;
        let mut dir = PathBuf::from(matches.value_of_os("load").unwrap());

        dir.push("users.toml");
        let map = db::user::load_file(&dir)?;
        for (uid,user) in map.iter() {
            db.userdb.put_user(uid, user)?;
        }
        debug!(log, "Loaded users: {:?}", map);
        dir.pop();

        Ok(())
    } else {
        let ex = Executor::new();
        let db = db::Databases::new(&log, &config)?;

        {
            info!(log, "Loaded DB state:");
            let txn = db.machine.txn()?;
            for (id, state) in db.machine.iter(&txn)? {
                info!(log, "- {}: {:?}", id, state);
            }
            info!(log, "Loaded DB state END.");
        }

        let machines = machine::load(&config, db.clone(), &log)?;
        let (actor_map, actors) = actor::load(&log, &config)?;
        let (init_map, initiators) = initiator::load(&log, &config)?;

        let mut network = network::Network::new(machines, actor_map, init_map);

        for ActorConn { machine, actor } in config.actor_connections.iter() {
            if let Err(e) = network.connect_actor(machine, actor) {
                error!(log, "{}", e);
            }
            info!(log, "[Actor] Connected {} to {}", machine, actor);
        }

        for InitiatorConn { initiator, machine } in config.init_connections.iter() {
            if let Err(e) = network.connect_init(initiator, machine) {
                error!(log, "{}", e);
            }
            info!(log, "[Initi] Connected {} to {}", initiator, machine);
        }

        for actor in actors.into_iter() {
            ex.spawn(actor).detach();
        }
        for init in initiators.into_iter() {
            ex.spawn(init).detach();
        }

        server::serve_api_connections(log.clone(), config, db, network, ex)
    }
}
