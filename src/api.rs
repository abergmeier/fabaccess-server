use std::rc::Rc;
use std::cell::RefCell;
use std::ops::Deref;

use slog::Logger;

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

mod user;
mod users;
use users::Users;

// TODO Session restoration by making the Bootstrap cap a SturdyRef
pub struct Bootstrap {
    log: Logger,

    db: Databases,
    nw: Arc<Network>,

    session: Rc<RefCell<Option<Session>>>,
}

impl Bootstrap {
    pub fn new(log: Logger, db: Databases, nw: Arc<Network>) -> Self {
        info!(log, "Created Bootstrap");
        let session = Rc::new(RefCell::new(None));
        Self { session, db, nw, log }
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

        // If this Rc has a strong count of 1 then there's no other cap issued yet meaning we can
        // safely transform the inner session with an auth.
        if Rc::strong_count(&self.session) == 1 {
            let session = Rc::clone(&self.session);
            let db = self.db.clone();
            res.get().set_authentication_system(capnp_rpc::new_client(
                    auth::Auth::new(self.log.new(o!()), db, session))
            );
        }

        Promise::ok(())
    }

    fn machine_system(&mut self,
        _: MachineSystemParams,
        mut res: MachineSystemResults
    ) -> Promise<(), capnp::Error> {
        if let Some(session) = self.session.borrow().deref() {
            debug!(self.log, "Giving MachineSystem cap to user {} with perms:", session.authzid);
            for r in session.perms.iter() {
                debug!(session.log, "   {}", r);
            }

            // TODO actual permission check and stuff
            //      Right now we only check that the user has authenticated at all.
            let c = capnp_rpc::new_client(Machines::new(Rc::clone(&self.session), self.nw.clone()));
            res.get().set_machine_system(c);
        }

        Promise::ok(())
    }

    fn user_system(
        &mut self,
        _: UserSystemParams,
        mut results: UserSystemResults
    ) -> Promise<(), capnp::Error> {
        if self.session.borrow().is_some() {
            // TODO actual permission check and stuff
            //      Right now we only check that the user has authenticated at all.
            let c = capnp_rpc::new_client(Users::new(Rc::clone(&self.session), self.db.userdb.clone()));
            results.get().set_user_system(c);
        }

        Promise::ok(())
    }
}
