use futures::FutureExt;
use std::future::Future;
use std::sync::Arc;

use slog::Logger;

use smol::lock::Mutex;
use smol::net::TcpStream;

use crate::error::Result;

use capnp_rpc::{rpc_twoparty_capnp, twoparty};

use crate::schema::connection_capnp;

use crate::db::access::{AccessControl, PermRule, RoleIdentifier};
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

    pub fn handle(&mut self, stream: TcpStream) -> impl Future<Output = Result<()>> {
        info!(self.log, "New connection from on {:?}", stream);
        let boots = Bootstrap::new(self.log.new(o!()), self.db.clone(), self.network.clone());
        unimplemented!();
        /*let rpc: connection_capnp::bootstrap::Client = capnp_rpc::new_client(boots);

        let network = twoparty::VatNetwork::new(
            stream.clone(),
            stream,
            rpc_twoparty_capnp::Side::Server,
            Default::default(),
        );
        let rpc_system = capnp_rpc::RpcSystem::new(Box::new(network), Some(rpc.client));

        // Convert the error type to one of our errors
        rpc_system.map(|r| r.map_err(Into::into))
        */
    }
}
