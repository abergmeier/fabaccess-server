use async_net::TcpListener;

use capnp_rpc::rpc_twoparty_capnp::Side;
use capnp_rpc::twoparty::VatNetwork;
use capnp_rpc::RpcSystem;
use executor::prelude::{Executor, GroupId, SupervisionRegistry};
use futures_rustls::server::TlsStream;
use futures_rustls::TlsAcceptor;
use futures_util::stream::FuturesUnordered;
use futures_util::{stream, AsyncRead, AsyncWrite, FutureExt, StreamExt};

use std::future::Future;
use std::io;

use std::net::{IpAddr, SocketAddr};

use crate::authentication::AuthenticationHandle;
use crate::session::SessionManager;

mod config;
pub use config::{Listen, TlsListen};

mod authenticationsystem;
mod connection;
mod machine;
mod machinesystem;
mod permissionsystem;
mod session;
mod user;
mod user_system;

pub struct APIServer {
    executor: Executor<'static>,
    sockets: Vec<TcpListener>,
    acceptor: TlsAcceptor,
    sessionmanager: SessionManager,
    authentication: AuthenticationHandle,
}

impl APIServer {
    pub fn new(
        executor: Executor<'static>,
        sockets: Vec<TcpListener>,
        acceptor: TlsAcceptor,
        sessionmanager: SessionManager,
        authentication: AuthenticationHandle,
    ) -> Self {
        Self {
            executor,
            sockets,
            acceptor,
            sessionmanager,
            authentication,
        }
    }

    pub async fn bind(
        executor: Executor<'static>,
        listens: impl IntoIterator<Item = &Listen>,
        acceptor: TlsAcceptor,
        sessionmanager: SessionManager,
        authentication: AuthenticationHandle,
    ) -> miette::Result<Self> {
        let span = tracing::info_span!("binding API listen sockets");
        let _guard = span.enter();

        let sockets = FuturesUnordered::new();

        listens
            .into_iter()
            .map(|a| async move { (async_net::resolve(a.to_tuple()).await, a) })
            .collect::<FuturesUnordered<_>>()
            .filter_map(|(res, addr)| async move {
                match res {
                    Ok(a) => Some(a),
                    Err(e) => {
                        tracing::error!("Failed to resolve {:?}: {}", addr, e);
                        None
                    }
                }
            })
            .for_each(|addrs| async {
                for addr in addrs {
                    sockets.push(async move { (TcpListener::bind(addr).await, addr) })
                }
            })
            .await;

        let sockets: Vec<TcpListener> = sockets
            .filter_map(|(res, addr)| async move {
                match res {
                    Ok(s) => {
                        tracing::info!("Opened listen socket on {}", addr);
                        Some(s)
                    }
                    Err(e) => {
                        tracing::error!("Failed to open socket on {}: {}", addr, e);
                        None
                    }
                }
            })
            .collect()
            .await;

        tracing::info!("listening on {:?}", sockets);

        if sockets.is_empty() {
            tracing::warn!("No usable listen addresses configured for the API server!");
        }

        Ok(Self::new(
            executor,
            sockets,
            acceptor,
            sessionmanager,
            authentication,
        ))
    }

    pub async fn handle_until(self, stop: impl Future) {
        stream::select_all(
            self.sockets
                .iter()
                .map(|tcplistener| tcplistener.incoming()),
        )
        .take_until(stop)
        .for_each(|stream| async {
            match stream {
                Ok(stream) => {
                    if let Ok(peer_addr) = stream.peer_addr() {
                        self.handle(peer_addr, self.acceptor.accept(stream))
                    } else {
                        tracing::error!(?stream, "failing a TCP connection with no peer addr");
                    }
                }
                Err(e) => tracing::warn!("Failed to accept stream: {}", e),
            }
        })
        .await;
        tracing::info!("closing down API handler");
    }

    fn handle<IO: 'static + Unpin + AsyncRead + AsyncWrite>(
        &self,
        peer_addr: SocketAddr,
        stream: impl Future<Output = io::Result<TlsStream<IO>>>,
    ) {
        let span = tracing::trace_span!("api.handle");
        let _guard = span.enter();

        struct Peer {
            ip: IpAddr,
            port: u16,
        }

        let peer = Peer {
            ip: peer_addr.ip(),
            port: peer_addr.port(),
        };
        tracing::debug!(
            %peer.ip,
            peer.port,
            "spawning api handler"
        );

        let connection_span = tracing::info_span!(
            "rpcsystem",
            %peer.ip,
            peer.port,
        );
        let f = async move {
            tracing::trace!(parent: &connection_span, "starting tls exchange");
            let stream = match stream.await {
                Ok(stream) => stream,
                Err(error) => {
                    tracing::error!(parent: &connection_span, %error, "TLS handshake failed");
                    return;
                }
            };
            let (rx, tx) = futures_lite::io::split(stream);
            let vat = VatNetwork::new(rx, tx, Side::Server, Default::default());

            let bootstrap: connection::Client = capnp_rpc::new_client(connection::BootCap::new(
                peer_addr,
                self.authentication.clone(),
                self.sessionmanager.clone(),
                connection_span.clone(),
            ));

            if let Err(error) = RpcSystem::new(Box::new(vat), Some(bootstrap.client)).await {
                tracing::error!(
                    parent: &connection_span,
                    %error,
                    "error occured during rpc handling",
                );
            }
        };
        let cgroup = SupervisionRegistry::with(SupervisionRegistry::new_group);
        self.executor.spawn_local_cgroup(f, cgroup);
    }
}
