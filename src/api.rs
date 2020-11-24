use std::sync::Arc;

use slog::Logger;

use capnp::capability::{Params, Results, Promise};

use crate::schema::connection_capnp;
use crate::connection::Session;

use crate::db::Databases;

use crate::builtin;

pub mod auth;
mod machine;
mod machines;

use machines::Machines;

pub struct Bootstrap {
    session: Arc<Session>,
    db: Databases,
}

impl Bootstrap {
    pub fn new(session: Arc<Session>, db: Databases) -> Self {
        info!(session.log, "Created Bootstrap");
        Self { session, db }
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

        res.get().set_auth(capnp_rpc::new_client(auth::Auth::new(self.session.clone())));

        Promise::ok(())
    }

    fn permissions(&mut self,
        _: Params<permissions_params::Owned>,
        _: Results<permissions_results::Owned>
    ) -> Promise<(), capnp::Error> {
        Promise::ok(())
    }

    fn machines(&mut self,
        _: Params<machines_params::Owned>,
        mut res: Results<machines_results::Owned>
    ) -> Promise<(), capnp::Error> {
        // TODO actual permission check and stuff
        let c = capnp_rpc::new_client(Machines::new(self.session.clone(), self.db.clone()));
        res.get().set_machines(c);

        Promise::ok(())
    }
}

