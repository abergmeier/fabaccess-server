use clap::{Arg, Command};
use diflouroborane::db::{Databases, Dump};
use diflouroborane::{config, Diflouroborane, error::Error};
use std::net::ToSocketAddrs;
use std::os::unix::prelude::AsRawFd;
use std::str::FromStr;
use std::{env, io, io::Write, path::PathBuf};
use anyhow::Context;
use nix::NixPath;

fn main() -> anyhow::Result<()> {
    // Argument parsing
    // values for the name, description and version are pulled from `Cargo.toml`.
    let matches = Command::new(clap::crate_name!())
        .version(clap::crate_version!())
        .about(clap::crate_description!())
        .arg(
            Arg::new("config")
                .help("Path to the config file to use")
                .long("config")
                .short('c')
                .takes_value(true),
        )
        .arg(Arg::new("verbosity")
            .help("Increase logging verbosity")
            .long("verbose")
            .short('v')
            .multiple_occurrences(true)
            .max_occurrences(3)
            .conflicts_with("quiet")
        )
        .arg(Arg::new("quiet")
            .help("Decrease logging verbosity")
            .long("quiet")
            .conflicts_with("verbosity")
        )
        .arg(Arg::new("log format")
            .help("Use an alternative log formatter. Available: Full, Compact, Pretty")
            .long("log-format")
            .takes_value(true)
            .ignore_case(true)
            .possible_values(["Full", "Compact", "Pretty"]))
        .arg(
            Arg::new("print default")
                .help("Print a default config to stdout instead of running")
                .long("print-default"),
        )
        .arg(
            Arg::new("check config")
                .help("Check config for validity")
                .long("check"),
        )
        .arg(
            Arg::new("dump")
                .help("Dump all internal databases")
                .long("dump")
                .conflicts_with("load"),
        )
        .arg(
            Arg::new("load")
                .help("Load values into the internal databases")
                .long("load")
                .conflicts_with("dump"),
        )
        .arg(Arg::new("keylog")
            .help("log TLS keys into PATH. If no path is specified the value of the envvar SSLKEYLOGFILE is used.")
            .long("tls-key-log")
            .value_name("PATH")
            .takes_value(true)
            .max_values(1)
            .min_values(0)
            .default_missing_value("")
        )
        .get_matches();

    let configpath = matches
        .value_of("config")
        .unwrap_or("/etc/diflouroborane.dhall");

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
    } else if matches.is_present("dump") {
        unimplemented!()
    } else if matches.is_present("load") {
        unimplemented!()
    } else {
        let keylog = matches.value_of("keylog");
        // When passed an empty string (i.e no value) take the value from the env
        let keylog = if let Some("") = keylog {
            let v = env::var_os("SSLKEYLOGFILE").map(PathBuf::from);
            if v.is_none() || v.as_ref().unwrap().is_empty() {
                eprintln!("--tls-key-log set but no path configured!");
                return Ok(());
            }
            v
        } else {
            keylog.map(PathBuf::from)
        };

        let mut config = config::read(&PathBuf::from_str(configpath).unwrap()).unwrap();

        config.tlskeylog = keylog;
        config.verbosity = matches.occurrences_of("verbosity") as isize;
        if config.verbosity == 0 && matches.is_present("quiet") {
            config.verbosity = -1;
        }
        config.log_format = matches.value_of("log format").unwrap_or("Full").to_string();

        let mut bffh = Diflouroborane::new();
        bffh.setup(&config)?;
    }

    Ok(())
}
