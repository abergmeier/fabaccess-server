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

use clap::{App, Arg};

use futures::prelude::*;
use futures::executor::{LocalPool, ThreadPool};
use futures::compat::Stream01CompatExt;
use futures::join;
use futures::task::LocalSpawn;

use smol::net::TcpListener;
use smol::net::unix::UnixStream;

use std::io;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;

use std::sync::Arc;

use lmdb::Transaction;

use error::Error;

use registries::Registries;

const LMDB_MAX_DB: u32 = 16;

// Returning a `Result` from `main` allows us to use the `?` shorthand.
// In the case of an Err it will be printed using `fmt::Debug`
fn main() -> Result<(), Error> {
    let signal = Box::pin(async {
        let (tx, mut rx) = UnixStream::pair()?;
        // Initialize signal handler.
        // We currently only care about Ctrl-C so SIGINT it is.
        // TODO: Make this do SIGHUP and a few others too. (By cloning the tx end of the pipe)
        signal_hook::pipe::register(signal_hook::SIGINT, tx)?;
        // When a signal is received this future can complete and read a byte from the underlying
        // socket — the actual data is discarded but the act of being able to receive data tells us
        // that we received a SIGINT.

        // FIXME: What errors are possible and how to handle them properly?
        rx.read_exact(&mut [0u8]).await?;

        io::Result::Ok(LoopResult::Stop)
    });

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

    // Initialize the LMDB environment. Since this would usually block untill the mmap() finishes
    // we wrap it in smol::unblock which runs this as future in a different thread.
    let e_config = config.clone();
    info!(log, "LMDB env");
    let env = lmdb::Environment::new()
        .set_flags(lmdb::EnvironmentFlags::MAP_ASYNC | lmdb::EnvironmentFlags::NO_SUB_DIR)
        .set_max_dbs(LMDB_MAX_DB as libc::c_uint)
        .open(&PathBuf::from_str("/tmp/a.db").unwrap())?;

    // Kick up an executor
    // Most initializations from now on do some amount of IO and are much better done in an
    // asyncronous fashion.
    let mut exec = LocalPool::new();


    // Start loading the machine database, authentication system and permission system
    // All of those get a custom logger so the source of a log message can be better traced and
    // filtered
    let env = Arc::new(env);
    let mdb = db::machine::init(log.new(o!("system" => "machines")), &config, env.clone());
    let pdb = db::access::init(log.new(o!("system" => "permissions")), &config, env.clone());

    // If --load or --dump is given we can stop at this point and load/dump the database and then
    // exit.
    if matches.is_present("load") {
        if let Some(pathstr) = matches.value_of("load") {
            let path = std::path::Path::new(pathstr);

            let mut txn = env.begin_rw_txn()?;
            let path = path.to_path_buf();
            pdb?.load_db(&mut txn, path.clone())?;
            //mdb?.load_db(&mut txn, path)?;
            txn.commit()?;
        } else {
            error!(log, "You must provide a directory path to load from");
        }

        return Ok(())
    } else if matches.is_present("dump") {
        if let Some(pathstr) = matches.value_of("dump") {
            let path = std::path::Path::new(pathstr);
            if let Err(e) = std::fs::create_dir_all(path) {
                error!(log, "The provided path could not be created: {}", e);
                return Ok(())
            }

            let txn = env.begin_ro_txn()?;
            let path = path.to_path_buf();
            pdb?.dump_db(&txn, path.clone())?;
            //mdb?.dump_db(&txn, path)?;
        } else {
            error!(log, "You must provide a directory path to dump into");
        }

        return Ok(())
    }


    // Bind to each address in config.listen.
    // This is a Stream over Futures so it will do absolutely nothing unless polled to completion
    let listeners_s: futures::stream::Collect<_, Vec<TcpListener>> 
        = stream::iter((&config).listens.iter())
        .map(|l| {
            let addr = l.address.clone();
            let port = l.port.unwrap_or(config::DEFAULT_PORT);
            TcpListener::bind((l.address.as_str(), port))
                // If the bind errors, include the address so we can log it
                // Since this closure is lazy we need to have a cloned addr
                .map_err(move |e| { (addr, port, e) })
        })
        .filter_map(|f| async {
            match f.await {
                Ok(l) => Some(l),
                Err((addr, port, e)) => {
                    error!(&log, "Could not setup socket on {} port {}: {}", addr, port, e);
                    None
                }
            }
        }).collect();

    //let (mach, auth) = exec.run_until(async {
    //    // Rull all futures to completion in parallel.
    //    // This will block until all three are done starting up.
    //    join!(machinedb_f, authentication_f)
    //});

    // Error out if any of the subsystems failed to start.
    let mdb = mdb?;
    let defs = machine::MachineDescription::load_file(&config.machines)?;
    let machdb = db::machine::MachineDB::new(mdb, defs);
    info!(log, "{:?}", machdb);
    let pdb = pdb?;
    let mut ac = db::access::AccessControl::new();
    ac.add_source_unchecked("Internal".to_string(), Box::new(pdb));
    let db = db::Databases {
        access: Arc::new(db::access::AccessControl::new()),
        machine: Arc::new(machdb),
    };

    // Since the below closures will happen at a much later time we need to make sure all pointers
    // are still valid. Thus, Arc.
    let start_log = log.clone();
    let stop_log = log.clone();

    // Create a thread pool to run tasks on
    let pool = ThreadPool::builder()
        .after_start(move |i| {
            info!(start_log.new(o!("system" => "threadpool")), "Starting Thread <{}>", i)
        })
        .before_stop(move |i| {
            info!(stop_log.new(o!("system" => "threadpool")), "Stopping Thread <{}>", i)
        })
        .create()?;
    let local_spawn = exec.spawner();

    // Start all modules on the threadpool. The pool will run the modules until it is dropped.
    // FIXME: implement notification so the modules can shut down cleanly instead of being killed
    // without warning.
    let modlog = log.clone();
    let mut regs = Registries::new();
    match exec.run_until(modules::init(modlog.new(o!("system" => "modules")), config.clone(), pool.clone(), regs.clone())) {
        Ok(()) => {}
        Err(e) => {
            error!(modlog, "Module startup failed: {}", e);
            return Err(e);
        }
    }

    // Closure inefficiencies. Lucky cloning an Arc is pretty cheap.
    let inner_log = log.clone();
    let loop_log = log.clone();

    exec.run_until(async move {
        // Generate a stream of TcpStreams appearing on any of the interfaces we listen to
        let listeners = listeners_s.await;
        let incoming = stream::select_all(listeners.iter().map(|l| l.incoming()));

        // For each incoming connection start a new task to handle it
        let handle_sockets = incoming.map(|socket| {
            // incoming.next() returns an error when the underlying `accept` call yielded an error
            // In POSIX those are protocol errors we can't really handle, so we just log the error
            // and the move on
            match socket {
                Ok(socket) => {
                    // If we have it available add the peer's address to all log messages
                    let log =
                        if let Ok(addr) = socket.peer_addr() {
                            inner_log.new(o!("address" => addr))
                        } else {
                            inner_log.new(o!())
                        };

                    // Clone a log for potential error handling
                    let elog = log.clone();

                    // We handle the error using map_err
                    let f = connection::handle_connection(log.clone(), socket, db.clone())
                        .map_err(move |e| {
                            error!(log, "Error occured during protocol handling: {}", e);
                        })
                        // Void any and all results since pool.spawn allows no return value.
                        .map(|_| ());

                    // In this case only the error is relevant since the Value is always ()
                    // The future is Boxed to make it the `LocalFutureObj` that LocalSpawn expects
                    if let Err(e) = local_spawn.spawn_local_obj(Box::new(f).into()) {
                        error!(elog, "Failed to spawn connection handler: {}", e);
                        // Failing to spawn a handler means we are most likely overloaded
                        return LoopResult::Overloaded;
                    }
                },
                Err(e) => {
                    error!(inner_log, "Socket `accept` error: {}", e);
                }
            }

            // Unless we are overloaded we just want to keep going.
            return LoopResult::Continue;
        });

        // Check each signal as it arrives
        let handle_signals = signal.map(|r| { r.unwrap() }).into_stream();

        let mut combined = stream::select(handle_signals, handle_sockets);

        // This is the basic main loop that drives execution
        loop {
            match combined.next().await {
                // When the result says to continue, do exactly that
                Some(LoopResult::Continue) => {}
                Some(LoopResult::Overloaded) => {
                    // In case over server overload we should install a replacement handler that
                    // would instead just return `overloaded` for all connections until the
                    // situation is remedied.
                    //
                    // For now, just log the overload and keep going.
                    error!(loop_log, "Server overloaded");
                }
                // When the result says to stop the server, do exactly that.
                // Also catches a `None` from the stream; None should never be returned because it
                // would mean all sockets were closed and we can not receive any further signals.
                // Still, in that case shut down cleanly anyway, the only reason this could happen
                // are some heavy bugs in the runtime
                Some(LoopResult::Stop) | None => {
                    warn!(loop_log, "Stopping server");
                    break;
                }
            }
        }
    });

    // TODO: Run actual shut down code here
    info!(log, "Shutting down...");

    // Returning () is an implicit success so this will properly set the exit code as well
    Ok(())
}

/// The result of one iteration of the core loop
enum LoopResult {
    /// Everything was fine, keep going
    Continue,
    /// Something happened that means we should shut down
    Stop,
    /// The Server is currently overloaded
    Overloaded,
}
