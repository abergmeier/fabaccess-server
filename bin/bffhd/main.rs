use clap::{Arg, Command, Parser};
use diflouroborane::{config, Diflouroborane};


use std::str::FromStr;
use std::{env, io, io::Write, path::PathBuf};



use nix::NixPath;

fn main() -> anyhow::Result<()> {
    // Argument parsing
    // values for the name, description and version are pulled from `Cargo.toml`.
    let matches = Command::new(clap::crate_name!())
        .version(clap::crate_version!())
        .long_version(diflouroborane::VERSION_STRING)
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
        .arg(Arg::new("log level")
            .help("Set the desired log levels.")
            .long("log-level")
            .takes_value(true))
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
                .takes_value(true)
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
        handle.write_all(encoded.as_bytes()).unwrap();

        // Early return to exit.
        return Ok(());
    } else if matches.is_present("check config") {
        match config::read(&PathBuf::from_str(configpath).unwrap()) {
            Ok(c) => {
                let formatted = format!("{:#?}", c);

                // Direct writing to fd 1 is faster but also prevents any print-formatting that could
                // invalidate the generated TOML
                let stdout = io::stdout();
                let mut handle = stdout.lock();
                handle.write_all(formatted.as_bytes()).unwrap();

                // Early return to exit.
                return Ok(());
            }
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(-1);
            }
        }
    }

    let mut config = config::read(&PathBuf::from_str(configpath).unwrap()).unwrap();

    if matches.is_present("dump") {
        unimplemented!()
    } else if matches.is_present("load") {
        let bffh = Diflouroborane::new(config)?;
        if bffh.users.load_file(matches.value_of("load").unwrap()).is_ok() {
            tracing::info!("loaded users from {}", matches.value_of("load").unwrap());
        } else {
            tracing::error!("failed to load users from {}", matches.value_of("load").unwrap());
        }
        return Ok(())
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

        config.tlskeylog = keylog;
        config.verbosity = matches.occurrences_of("verbosity") as isize;
        if config.verbosity == 0 && matches.is_present("quiet") {
            config.verbosity = -1;
        }
        config.logging.format = matches.value_of("log format").unwrap_or("full").to_string();

        let mut bffh = Diflouroborane::new(config)?;
        bffh.run()?;
    }

    Ok(())
}
