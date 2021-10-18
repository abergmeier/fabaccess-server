// FIXME: No.
#![allow(dead_code)]
#![forbid(unused_imports)]

/*
mod modules;
mod log;
mod config;
mod connection;
mod db;
mod machine;
mod builtin;
mod server;
mod network;
mod actor;
mod initiator;
mod space;
*/

use rkyv::ser::serializers::AllocSerializer;
use rkyv::{SerializeUnsized, archived_value, Infallible, Deserialize};
use crate::oid::ObjectIdentifier;
use std::str::FromStr;

mod resource;
mod schema;
mod state;
mod db;
mod network;
pub mod oid;
mod varint;

/*

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
use crate::config::Config;
*/

pub fn main() {
    let db = db::StateDB::init("/tmp/state").unwrap();
    println!("{:#?}", db);

    let b = true;
    //println!("{}", b.archived_type_oid());

    let boid = &state::value::OID_BOOL;
    let b2 = false;
    let ent = state::Entry { oid: boid, val: &b2 };
    println!("ent {:?}", &ent);
    let s = serde_json::to_string(&ent).unwrap();
    println!("{}", &s);
    let ent2: state::OwnedEntry = serde_json::from_str(&s).unwrap();
    println!("ent2: {:?}", ent2);

    println!("Hello");

    let mut ser = AllocSerializer::<32>::default();
    //let b3 = Box::new(u32::from_ne_bytes([0xDE, 0xAD, 0xBE, 0xEF])) as Box<dyn state::value::SerializeValue>;
    let b3 = Box::new(true) as Box<dyn state::value::SerializeValue>;
    let pos3 = b3.serialize_unsized(&mut ser).unwrap();
    let pos4 = 0;
    //let pos4 = b4.serialize_unsized(&mut ser).unwrap();
    let buf = ser.into_serializer().into_inner();
    println!("({}) {:?} | {:?}", pos3, &buf[..pos3], &buf[pos3..]);
    //println!("Serialized {} bytes: {:?} | {:?} | {:?}", pos4, &buf[..pos3], &buf[pos3+12..pos4], &buf[pos4+12..]);
    let r3 = unsafe {
        archived_value::<Box<dyn state::value::SerializeValue>>(&buf.as_slice(), pos3)
    };
    let v3: Box<dyn state::value::SerializeValue> = r3.deserialize(&mut Infallible).unwrap();
    println!("{:?}", v3);

    let koid = ObjectIdentifier::from_str("1.3.6.1.4.1.48398.612.2.1").unwrap();
    let state = state::State::build()
        .add(koid, Box::new(0xDEADBEEFu32))
        .finish();

    println!("{:?}", state);
    let json = serde_json::to_string(&state).unwrap();
    println!("{}", json);
    let state_back: state::State = serde_json::from_str(&json).unwrap();
    println!("{:?}", state_back);
    let val = state_back.inner;
}

/*fn main() {
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

        let machines = machine::load(&config)?;
        let (actor_map, actors) = actor::load(&log, &config)?;
        let (init_map, initiators) = initiator::load(&log, &config, db.userdb.clone(), db.access.clone())?;

        let mut network = network::Network::new(machines, actor_map, init_map);

        for (a,b) in config.actor_connections.iter() {
            if let Err(e) = network.connect_actor(a,b) {
                error!(log, "{}", e);
            }
            info!(log, "[Actor] Connected {} to {}", a, b);
        }

        for (a,b) in config.init_connections.iter() {
            if let Err(e) = network.connect_init(a,b) {
                error!(log, "{}", e);
            }
            info!(log, "[Initi] Connected {} to {}", a, b);
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
*/
