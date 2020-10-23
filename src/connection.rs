use slog::Logger;

use smol::net::TcpStream;

use crate::error::Result;
use crate::auth;
use crate::api;

pub use crate::schema::connection_capnp;

use capnp::capability::{Params, Results, Promise, FromServer};

/// Connection context
struct Connection {
    stream: TcpStream,
    user: Option<auth::User>,
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

pub async fn handle_connection(log: Logger, mut stream: TcpStream) -> Result<()> {
    unimplemented!()
}
