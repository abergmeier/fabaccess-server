use std::fs::File;
use slog::Logger;

use crate::config;
use crate::config::Config;
use crate::error::Error;
use crate::connection;

use smol::net::TcpListener;
use smol::net::unix::UnixStream;
use smol::LocalExecutor;
use smol::Executor;

use futures::prelude::*;

use std::io;
use std::io::BufReader;

use std::sync::Arc;

use std::os::unix::io::AsRawFd;
use std::path::Path;
use async_rustls::TlsAcceptor;
use rustls::{Certificate, KeyLogFile, NoClientAuth, PrivateKey, ServerConfig};
use rustls_pemfile::Item;
use signal_hook::low_level::pipe as sigpipe;

use crate::db::Databases;
use crate::network::Network;

/// Handle all API connections and run the RPC tasks spawned from that on the local thread.
pub fn serve_api_connections(log: Arc<Logger>, config: Config, db: Databases, nw: Network, ex: Executor)
    -> Result<(), Error> 
{
    let signal = Box::pin(async {
        use signal_hook::consts::signal::*;
        let (tx, mut rx) = UnixStream::pair()?;
        // Initialize signal handler.
        // We currently only care about Ctrl-C so SIGINT it is.
        // TODO: Make this do SIGHUP and a few others too. (By cloning the tx end of the pipe)
        let fd = tx.as_raw_fd();
        sigpipe::register(SIGINT, fd)?;
        sigpipe::register(SIGQUIT, fd)?;
        sigpipe::register(SIGTERM, fd)?;
        // When a signal is received this future can complete and read a byte from the underlying
        // socket — the actual data is discarded but the act of being able to receive data tells us
        // that we received a SIGINT.

        // FIXME: What errors are possible and how to handle them properly?
        rx.read_exact(&mut [0u8]).await?;

        io::Result::Ok(LoopResult::Stop)
    });

    info!(log, "Reading certificate chain file");
    let mut certfp = BufReader::new(File::open(&config.certfile)?);
    let certs = rustls_pemfile::certs(&mut certfp)?
        .into_iter()
        .map(Certificate)
        .collect();
    info!(log, "Reading private key file");
    let mut keyfp = BufReader::new(File::open(&config.keyfile)?);
    let mut tls_config = ServerConfig::new(Arc::new(NoClientAuth));
    tls_config.key_log = Arc::new(KeyLogFile::new());
    if let Some(path) = std::env::var_os("SSLKEYLOGFILE") {
        let path = Path::new(&path);
        warn!(log, "TLS SECRET LOGGING ENABLED! This will write all connection secrets to file {}!",
            path.display());
    }
    match rustls_pemfile::read_one(&mut keyfp)? {
        Some(rustls_pemfile::Item::PKCS8Key(key) | rustls_pemfile::Item::RSAKey(key)) => {
            let key = PrivateKey(key);
            tls_config.set_single_cert(certs, key)?;
        }
        _ => {
            error!(log, "private key file must contain a PEM-encoded private key");
            return Ok(());
        }
    }
    let tls_acceptor: TlsAcceptor = Arc::new(tls_config).into();

    // Bind to each address in config.listens.
    // This is a Stream over Futures so it will do absolutely nothing unless polled to completion
    let listeners_s: futures::stream::Collect<_, Vec<TcpListener>> 
        = stream::iter((&config).listens.iter())
        .map(|l| {
            let addr = l.address.clone();
            let port = l.port.unwrap_or(config::DEFAULT_PORT);
            info!(&log, "Binding to {} port {}.", l.address.as_str(), &port);
            TcpListener::bind((l.address.as_str(), port))
                // If the bind errors, include the address so we can log it
                // Since this closure is lazy we need to have a cloned addr
                .map_err(move |e| { (addr, port, e) })
        })
        // Filter out the sockets we couldn't open and log those
        .filter_map(|f| async {
            match f.await {
                Ok(l) => Some(l),
                Err((addr, port, e)) => {
                    error!(&log, "Could not setup socket on {} port {}: {}", addr, port, e);
                    None
                }
            }
        }).collect();

    let local_ex = LocalExecutor::new();

    let network = Arc::new(nw);

    let inner_log = log.clone();
    let loop_log = log.clone();

    let control_fut = async {
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


                    let db = db.clone();
                    let network = network.clone();
                    let tlog = inner_log.new(o!());

                    let tls_acceptor_clone = tls_acceptor.clone();
                    std::thread::spawn(move || {
                        let tls_acceptor = tls_acceptor_clone;

                        info!(tlog, "New connection from on {:?}", socket);
                        let peer = socket.peer_addr();
                        let mut handler = connection::ConnectionHandler::new(tlog, db, network);
                        // We handle the error using map_err
                        let f = tls_acceptor.accept(socket)
                            .map_err(Error::IO)
                            .and_then(|stream| handler.handle(stream));

                        // Spawn the connection context onto the local executor since it isn't Send
                        // Also `detach` it so the task isn't canceled as soon as it's dropped.
                        // TODO: Store all those tasks to have a easier way of managing them?
                        if let Err(e) = smol::block_on(f) {
                            error!(log, "Error occurred during connection handling: {:?}", e)
                        } else if let Ok(peer) = peer {
                            debug!(log, "Closed connection with {:?}", peer);
                        } else {
                            debug!(log, "Closed connection with unknown peer");
                        }
                    });
                },
                Err(e) => {
                    error!(inner_log, "Socket `accept` error: {}", e);
                }
            }

            // Unless we are overloaded we just want to keep going.
            return LoopResult::Continue;
        });

        info!(&log, "Started");

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
    };

    smol::block_on(smol::future::race(control_fut, ex.run(smol::future::pending())));

    // TODO: Run actual shut down code here
    info!(log, "Shutting down...");

    // Returning () is an implicit success so this will properly set the exit code as well
    Ok(())
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
