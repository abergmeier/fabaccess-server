use slog::Logger;

use smol::net::TcpStream;

use crate::error::{Error, Result};
use crate::auth;
use crate::api;

pub use crate::schema::connection_capnp;

use capnp_rpc::{twoparty, rpc_twoparty_capnp};

use capnp::capability::{Params, Results, Promise, FromServer};

/// Connection context
struct Connection {
    log: Logger,
    user: Option<auth::User>,
}

impl Connection {
    pub fn new(log: Logger) -> Self {
        let user = None;

        Self { log, user }
    }
}


use connection_capnp::bootstrap::*;
impl connection_capnp::bootstrap::Server for Connection {
    fn auth(&mut self, 
        _: Params<auth_params::Owned>,
        mut res: Results<auth_results::Owned>
    ) -> Promise<(), capnp::Error> {
        // Forbid mutltiple authentication for now
        // TODO: When should we allow multiple auth and how do me make sure that does not leak
        // priviledges (e.g. due to previously issues caps)?
        if self.user.is_none() {
            res.get().set_auth(capnp_rpc::new_client(auth::Auth::new()))
        }

        Promise::ok(())
    }

    fn permissions(&mut self,
        _: Params<permissions_params::Owned>,
        mut res: Results<permissions_results::Owned>
    ) -> Promise<(), capnp::Error> {
        if self.user.is_some() {
        }

        Promise::ok(())
    }
}

async fn handshake(log: &Logger, stream: &mut TcpStream) -> Result<()> {
    if let Some(m) = capnp_futures::serialize::read_message(stream, Default::default()).await? {
        let greeting = m.get_root::<connection_capnp::greeting::Reader>()?;
        let major = greeting.get_major();
        let minor = greeting.get_minor();

        if major != 1 {
            Err(Error::BadVersion((major, minor)))
        } else {
            info!(log, "Handshake successful with peer {} running {}, API {}.{}", 
                greeting.get_host()?, greeting.get_program()?, major, minor);
            Ok(())
        }
    } else {
        unimplemented!()
    }
}

pub async fn handle_connection(log: Logger, mut stream: TcpStream) -> Result<()> {
    handshake(&log, &mut stream).await?;

    let mut conn = Connection::new(log);
    let rpc: connection_capnp::bootstrap::Client = capnp_rpc::new_client(conn);

    let network = twoparty::VatNetwork::new(stream.clone(), stream,
        rpc_twoparty_capnp::Side::Server, Default::default());
    let rpc_system = capnp_rpc::RpcSystem::new(Box::new(network), 
        Some(rpc.client));

    rpc_system.await;
    Ok(())
}
