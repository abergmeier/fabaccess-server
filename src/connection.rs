use std::fmt::Debug;
use std::ops::DerefMut;
use futures::{AsyncRead, AsyncWrite, FutureExt};
use std::future::Future;
use std::io::{IoSlice, IoSliceMut};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll};
use async_rustls::server::TlsStream;

use slog::Logger;

use smol::lock::Mutex;

use crate::api::Bootstrap;
use crate::error::Result;

use capnp_rpc::{rpc_twoparty_capnp, twoparty};
use futures_util::{pin_mut, ready};

use crate::schema::connection_capnp;

use crate::db::access::{PermRule, RoleIdentifier};
use crate::db::user::UserId;
use crate::db::Databases;
use crate::network::Network;

#[derive(Debug)]
/// Connection context
// TODO this should track over several connections
pub struct Session {
    // Session-spezific log
    pub log: Logger,

    /// User this session has been authorized as.
    ///
    /// Slightly different than the authnid which indicates as what this session has been
    /// authenticated as (e.g. using EXTERNAL auth the authnid would be the CN of the client
    /// certificate, but the authzid would be an user)
    pub authzid: UserId,

    pub authnid: String,

    /// Roles this session has been assigned via group memberships, direct role assignment or
    /// authentication types
    pub roles: Box<[RoleIdentifier]>,

    /// Permissions this session has.
    ///
    /// This is a snapshot of the permissions the underlying user has
    /// take at time of creation (i.e. session establishment)
    pub perms: Box<[PermRule]>,
}

impl Session {
    pub fn new(
        log: Logger,
        authzid: UserId,
        authnid: String,
        roles: Box<[RoleIdentifier]>,
        perms: Box<[PermRule]>,
    ) -> Self {
        Session {
            log,
            authzid,
            authnid,
            roles,
            perms,
        }
    }
}

pub struct ConnectionHandler {
    log: Logger,
    db: Databases,
    network: Arc<Network>,
}

impl ConnectionHandler {
    pub fn new(log: Logger, db: Databases, network: Arc<Network>) -> Self {
        Self { log, db, network }
    }

    pub fn handle<IO: 'static + Unpin + AsyncWrite + AsyncRead>(&mut self, stream: TlsStream<IO>)
        -> impl Future<Output=Result<()>>
    {
        let conn = Connection::new(stream);

        let boots = Bootstrap::new(self.log.new(o!()), self.db.clone(), self.network.clone());
        let rpc: connection_capnp::bootstrap::Client = capnp_rpc::new_client(boots);

        let network = twoparty::VatNetwork::new(
            conn.clone(),
            conn,
            rpc_twoparty_capnp::Side::Server,
            Default::default(),
        );
        let rpc_system = capnp_rpc::RpcSystem::new(Box::new(network), Some(rpc.client));

        // Convert the error type to one of our errors
        rpc_system.map(|r| r.map_err(Into::into))
    }
}

struct Connection<IO> {
    inner: Rc<Mutex<TlsStream<IO>>>,
}

impl<IO> Connection<IO> {
    pub fn new(stream: TlsStream<IO>) -> Self {
        Self {
            inner: Rc::new(Mutex::new(stream)),
        }
    }
}

impl<IO> Clone for Connection<IO> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone()
        }
    }
}


impl<IO: 'static + AsyncRead + AsyncWrite + Unpin> AsyncRead for Connection<IO> {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<std::io::Result<usize>> {
        let f = self.inner.lock();
        pin_mut!(f);
        let mut guard = ready!(f.poll(cx));
        let stream = guard.deref_mut();
        Pin::new(stream).poll_read(cx, buf)
    }

    fn poll_read_vectored(self: Pin<&mut Self>, cx: &mut Context<'_>, bufs: &mut [IoSliceMut<'_>]) -> Poll<std::io::Result<usize>> {
        let f = self.inner.lock();
        pin_mut!(f);
        let mut guard = ready!(f.poll(cx));
        let stream = guard.deref_mut();
        Pin::new(stream).poll_read_vectored(cx, bufs)
    }
}

impl<IO: 'static + AsyncWrite + AsyncRead + Unpin> AsyncWrite for Connection<IO> {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<std::io::Result<usize>> {
        let f = self.inner.lock();
        pin_mut!(f);
        let mut guard = ready!(f.poll(cx));
        let stream = guard.deref_mut();
        Pin::new(stream).poll_write(cx, buf)
    }

    fn poll_write_vectored(self: Pin<&mut Self>, cx: &mut Context<'_>, bufs: &[IoSlice<'_>]) -> Poll<std::io::Result<usize>> {
        let f = self.inner.lock();
        pin_mut!(f);
        let mut guard = ready!(f.poll(cx));
        let stream = guard.deref_mut();
        Pin::new(stream).poll_write_vectored(cx, bufs)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let f = self.inner.lock();
        pin_mut!(f);
        let mut guard = ready!(f.poll(cx));
        let stream = guard.deref_mut();
        Pin::new(stream).poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let f = self.inner.lock();
        pin_mut!(f);
        let mut guard = ready!(f.poll(cx));
        let stream = guard.deref_mut();
        Pin::new(stream).poll_close(cx)
    }
}