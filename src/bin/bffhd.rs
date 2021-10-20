use std::{
    io,
    io::Write,
    path::PathBuf,
};
use clap::{App, Arg, crate_version, crate_description, crate_name};
use std::str::FromStr;
use diflouroborane::{config, error::Error};
use std::net::ToSocketAddrs;

fn main_res() -> Result<(), Error> {
    // Argument parsing
    // values for the name, description and version are pulled from `Cargo.toml`.
    let matches = App::new(crate_name!())
        .about(crate_description!())
        .version(crate_version!())
        .arg(Arg::with_name("config")
             .help("Path to the config file to use")
             .long("config")
             .short("c")
             .takes_value(true
             ))
         .arg(Arg::with_name("print default")
             .help("Print a default config to stdout instead of running")
             .long("print-default")
         )
         .arg(Arg::with_name("check config")
             .help("Check config for validity")
             .long("check")
         )
         .arg(Arg::with_name("dump")
             .help("Dump all internal databases")
             .long("dump")
             .conflicts_with("load")
         )
         .arg(Arg::with_name("load")
             .help("Load values into the internal databases")
             .long("load")
             .conflicts_with("dump")
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
        return Ok(());
    } else if matches.is_present("check config") {
        let configpath = matches.value_of("config").unwrap_or("/etc/diflouroborane.dhall");
        match config::read(&PathBuf::from_str(configpath).unwrap()) {
            Ok(_) => {
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

    // If no `config` option is given use a preset default.
    let configpath = matches.value_of("config").unwrap_or("/etc/diflouroborane.dhall");
    let config = config::read(&PathBuf::from_str(configpath).unwrap())?;
    println!("{:#?}", config);

    let mut sockaddrs = Vec::new();
    for listen in config.listens {
        match listen.to_socket_addrs() {
            Ok(addrs) => {
                sockaddrs.extend(addrs)
            },
            Err(e) => {
                tracing::error!("Invalid listen \"{}\" {}", listen, e);
            }
        }
    }

    println!("Final listens: {:?}", sockaddrs);

    /*
    if matches.is_present("dump") {
        let db = db::Databases::new(&log, &config)?;
        let v = db.access.dump_roles().unwrap();
        for (id, role) in v.iter() {
            tracing::info!("Role {}:\n{}", id, role);
        }

        let v = db.userdb.list_users()?;
        for user in v.iter() {
            tracing::info!("User {}:\n{:?}", user.id, user.data);
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
        tracing::debug!("Loaded users: {:?}", map);
        dir.pop();

        Ok(())
    } else {
        let ex = smol::Executor::new();
        let db = db::Databases::new(&log, &config)?;

        let machines = machine::load(&config)?;
        let (actor_map, actors) = actor::load(&log, &config)?;
        let (init_map, initiators) = initiator::load(&log, &config, db.userdb.clone(), db.access.clone())?;

        let mut network = network::Network::new(machines, actor_map, init_map);

        for (a,b) in config.actor_connections.iter() {
            if let Err(e) = network.connect_actor(a,b) {
                tracing::error!("{}", e);
            }
            tracing::info!("[Actor] Connected {} to {}", a, b);
        }

        for (a,b) in config.init_connections.iter() {
            if let Err(e) = network.connect_init(a,b) {
                tracing::error!("{}", e);
            }
            tracing::info!("[Initi] Connected {} to {}", a, b);
        }

        for actor in actors.into_iter() {
            ex.spawn(actor).detach();
        }
        for init in initiators.into_iter() {
            ex.spawn(init).detach();
        }

        server::serve_api_connections(log.clone(), config, db, network, ex)
    }
     */

    Ok(())
}

fn main() {
    let retval;
    // Scope to drop everything before exiting.
    {
        tracing_subscriber::fmt::init();
        match main_res() {
            Ok(_) => retval = 0,
            Err(e) => {
                tracing::error!("{}", e);
                retval = -1;
            }
        }
    }
    std::process::exit(retval);
}