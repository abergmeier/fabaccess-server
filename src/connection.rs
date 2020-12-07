use std::sync::Arc;
use std::future::Future;
use futures::FutureExt;

use slog::Logger;

use smol::net::TcpStream;

use crate::error::{Error, Result};
use crate::api::auth;
use crate::api::Bootstrap;

use capnp_rpc::{twoparty, rpc_twoparty_capnp};

use crate::schema::connection_capnp;

use crate::db::Databases;
use crate::db::access::{AccessControl, Permission};
use crate::db::user::User;
use crate::builtin;

#[derive(Debug, Clone)]
/// Connection context
// TODO this should track over several connections
pub struct Session {
    // Session-spezific log
    pub log: Logger,
    user: Option<User>,
    accessdb: Arc<AccessControl>,
}

impl Session {
    pub fn new(log: Logger, accessdb: Arc<AccessControl>) -> Self {
        let user = None;

        Session { log, user, accessdb }
    }

    /// Check if the current session has a certain permission
    pub async fn check_permission<P: AsRef<Permission>>(&self, perm: &P) -> Result<bool> {
        if let Some(user) = self.user.as_ref() {
            self.accessdb.check(&user.data, perm).await
        } else {
            Ok(false)
        }
    }
}

pub struct ConnectionHandler {
    log: Logger,
    db: Databases,
}

impl ConnectionHandler {
    pub fn new(log: Logger, db: Databases) -> Self {
        Self { log, db }
    }

    pub fn handle(&mut self, mut stream: TcpStream) -> impl Future<Output=Result<()>> {
        info!(self.log, "New connection from on {:?}", stream);
        let session = Arc::new(Session::new(self.log.new(o!()), self.db.access.clone()));
        let boots = Bootstrap::new(session, self.db.clone());
        let rpc: connection_capnp::bootstrap::Client = capnp_rpc::new_client(boots);

        let network = twoparty::VatNetwork::new(stream.clone(), stream,
            rpc_twoparty_capnp::Side::Server, Default::default());
        let rpc_system = capnp_rpc::RpcSystem::new(Box::new(network), Some(rpc.client));

        // Convert the error type to one of our errors
        rpc_system.map(|r| r.map_err(Into::into))
    }
}
