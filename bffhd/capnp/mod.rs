use crate::config::Listen;
use crate::{Diflouroborane, TlsConfig};
use anyhow::Context;
use async_net::TcpListener;
use capnp::capability::Promise;
use capnp::Error;
use capnp_rpc::rpc_twoparty_capnp::Side;
use capnp_rpc::twoparty::VatNetwork;
use capnp_rpc::RpcSystem;
use executor::prelude::Executor;
use futures_rustls::server::TlsStream;
use futures_rustls::TlsAcceptor;
use futures_util::stream::FuturesUnordered;
use futures_util::{stream, AsyncRead, AsyncWrite, FutureExt, StreamExt};
use std::fs::File;
use std::future::Future;
use std::io;
use std::io::BufReader;
use std::net::SocketAddr;
use std::sync::Arc;
use nix::sys::socket::SockAddr;
use crate::authentication::AuthenticationHandle;
use crate::authorization::AuthorizationHandle;

use crate::error::Result;
use crate::session::SessionManager;

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
    ) -> anyhow::Result<Self> {
        let span = tracing::info_span!("binding API listen sockets");
        let _guard = span.enter();

        let mut sockets = FuturesUnordered::new();

        listens
            .into_iter()
            .map(|a| async move {
                (async_net::resolve(a.to_tuple()).await, a)
            })
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

        Ok(Self::new(executor, sockets, acceptor, sessionmanager, authentication))
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
                },
                Err(e) => tracing::warn!("Failed to accept stream: {}", e),
            }
        }).await;
        tracing::info!("closing down API handler");
    }

    fn handle<IO: 'static + Unpin + AsyncRead + AsyncWrite>(
        &self,
        peer_addr: SocketAddr,
        stream: impl Future<Output = io::Result<TlsStream<IO>>>,
    ) {
        tracing::debug!("handling new API connection");
        let f = async move {
            let stream = match stream.await {
                Ok(stream) => stream,
                Err(e) => {
                    tracing::error!("TLS handshake failed: {}", e);
                    return;
                }
            };
            let (rx, tx) = futures_lite::io::split(stream);
            let vat = VatNetwork::new(rx, tx, Side::Server, Default::default());

            let bootstrap: connection::Client = capnp_rpc::new_client(connection::BootCap::new(peer_addr, self.authentication.clone(), self.sessionmanager.clone()));

            if let Err(e) = RpcSystem::new(Box::new(vat), Some(bootstrap.client)).await {
                tracing::error!("Error during RPC handling: {}", e);
            }
        };
        self.executor.spawn_local(f);
    }
}
