use std::sync::Arc;

use capnp::capability::{Params, Results, Promise};

use crate::schema::connection_capnp;
use crate::connection::Session;

use crate::db::Databases;

use crate::network::Network;

pub mod auth;
mod machine;
mod machines;

use machines::Machines;

// TODO Session restoration by making the Bootstrap cap a SturdyRef
pub struct Bootstrap {
    session: Arc<Session>,
    db: Databases,
    nw: Arc<Network>,
}

impl Bootstrap {
    pub fn new(session: Arc<Session>, db: Databases, nw: Arc<Network>) -> Self {
        info!(session.log, "Created Bootstrap");
        Self { session, db, nw }
    }
}

use connection_capnp::bootstrap::*;
impl connection_capnp::bootstrap::Server for Bootstrap {
    fn authentication_system(&mut self, 
        _: AuthenticationSystemParams,
        mut res: AuthenticationSystemResults
    ) -> Promise<(), capnp::Error> {
        // TODO: Forbid mutltiple authentication for now
        // TODO: When should we allow multiple auth and how do me make sure that does not leak
        // priviledges (e.g. due to previously issues caps)?

        res.get().set_authentication_system(capnp_rpc::new_client(auth::Auth::new(self.db.clone(), self.session.clone())));

        Promise::ok(())
    }

    fn permission_system(&mut self,
        _: PermissionSystemParams,
        _: PermissionSystemResults
    ) -> Promise<(), capnp::Error> {
        Promise::ok(())
    }

    fn machine_system(&mut self,
        _: MachineSystemParams,
        mut res: MachineSystemResults
    ) -> Promise<(), capnp::Error> {
        // TODO actual permission check and stuff
        let c = capnp_rpc::new_client(Machines::new(self.session.clone(), self.db.clone(), self.nw.clone()));
        res.get().set_machine_system(c);

        Promise::ok(())
    }
}

