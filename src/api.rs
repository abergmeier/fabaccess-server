use std::sync::Arc;

use capnp::capability::{Params, Results, Promise, FromServer};

use crate::schema::connection_capnp;
use crate::connection::Session;

pub mod auth;
mod machine;
mod machines;

use machines::Machines;

pub struct Bootstrap {
    session: Arc<Session>
}

impl Bootstrap {
    pub fn new(session: Arc<Session>) -> Self {
        Self { session }
    }
}

use connection_capnp::bootstrap::*;
impl connection_capnp::bootstrap::Server for Bootstrap {
    fn auth(&mut self, 
        _: Params<auth_params::Owned>,
        mut res: Results<auth_results::Owned>
    ) -> Promise<(), capnp::Error> {
        // Forbid mutltiple authentication for now
        // TODO: When should we allow multiple auth and how do me make sure that does not leak
        // priviledges (e.g. due to previously issues caps)?
        if self.session.user.is_none() {
            res.get().set_auth(capnp_rpc::new_client(auth::Auth::new()))
        }

        Promise::ok(())
    }

    fn permissions(&mut self,
        _: Params<permissions_params::Owned>,
        mut res: Results<permissions_results::Owned>
    ) -> Promise<(), capnp::Error> {
        if self.session.user.is_some() {
        }

        Promise::ok(())
    }

    fn machines(&mut self,
        _: Params<machines_params::Owned>,
        mut res: Results<machines_results::Owned>
    ) -> Promise<(), capnp::Error> {
        // TODO actual permission check and stuff
        if self.session.user.is_some() {
            let c = capnp_rpc::new_client(Machines::new(self.session.clone()));
            res.get().set_machines(c);
        }

        Promise::ok(())
    }
}

