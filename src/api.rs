use std::sync::Arc;

use capnp::capability::{Params, Results, Promise};

use crate::schema::connection_capnp;
use crate::connection::Session;

use crate::db::Databases;
use crate::db::user::UserId;

use crate::network::Network;

pub mod auth;
mod machine;
mod machines;
use machines::Machines;

mod users;
use users::Users;

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

    fn machine_system(&mut self,
        _: MachineSystemParams,
        mut res: MachineSystemResults
    ) -> Promise<(), capnp::Error> {
        let session = self.session.clone();
        let accessdb = self.db.access.clone();
        let nw = self.nw.clone();
        let f = async move {
            // Ensure the lock is dropped as soon as possible
            if let Some(user) = { session.user.lock().await.clone() } {
                let perms = accessdb.collect_permrules(&user.data)
                    .map_err(|e| capnp::Error::failed(format!("AccessDB lookup failed: {}", e)))?;

                debug!(session.log, "Giving MachineSystem cap to user {} with perms:", user.id);
                for r in perms.iter() {
                    debug!(session.log, "   {}", r);
                }

                // TODO actual permission check and stuff
                //      Right now we only check that the user has authenticated at all.
                let c = capnp_rpc::new_client(Machines::new(user.id, perms, nw));
                res.get().set_machine_system(c);
            }

            // Promise is Ok either way, just the machine system may not be set, indicating as
            // usual a lack of permission.
            Ok(())
        };

        Promise::from_future(f)
    }

    fn user_system(
        &mut self,
        _: UserSystemParams,
        mut results: UserSystemResults
    ) -> Promise<(), capnp::Error> {
        let session = self.session.clone();
        let accessdb = self.db.access.clone();
        let f = async move {
            // Ensure the lock is dropped as soon as possible
            if let Some(user) = { session.user.lock().await.clone() } {
                let perms = accessdb.collect_permrules(&user.data)
                    .map_err(|e| capnp::Error::failed(format!("AccessDB lookup failed: {}", e)))?;

                // TODO actual permission check and stuff
                //      Right now we only check that the user has authenticated at all.
                let c = capnp_rpc::new_client(Users::new(perms));
                results.get().set_user_system(c);
            }

            // Promise is Ok either way, just the machine system may not be set, indicating as
            // usual a lack of permission.
            Ok(())
        };

        Promise::from_future(f)
    }
}
