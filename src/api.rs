use std::rc::Rc;
use std::cell::RefCell;
use std::ops::Deref;

use slog::Logger;

use std::sync::Arc;

use capnp::capability::{Promise};
use rsasl::mechname::Mechname;
use rsasl::SASL;
use auth::State;

use crate::schema::connection_capnp;
use crate::connection::Session;

use crate::db::Databases;


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
    ctx: SASL,
}

impl Bootstrap {
    pub fn new(log: Logger, db: Databases, nw: Arc<Network>) -> Self {
        info!(log, "Created Bootstrap");
        let mut ctx = SASL::new();
        ctx.install_callback(Arc::new(auth::CB::new(db.userdb.clone())));
        Self { db, nw, log, ctx }
    }
}

use connection_capnp::{API_VERSION_MAJOR, API_VERSION_MINOR, API_VERSION_PATCH};
use connection_capnp::bootstrap::*;
use crate::api::auth::Auth;
use crate::RELEASE;

impl connection_capnp::bootstrap::Server for Bootstrap {
    fn get_a_p_i_version(
        &mut self,
        _: GetAPIVersionParams,
        _: GetAPIVersionResults,
    ) -> Promise<(), ::capnp::Error> {
        Promise::ok(())
    }

    fn get_server_release(
        &mut self,
        _: GetServerReleaseParams,
        mut result: GetServerReleaseResults,
    ) -> Promise<(), ::capnp::Error> {
        let mut builder = result.get();
        builder.set_name("bffhd");
        builder.set_release(crate::RELEASE);
        Promise::ok(())
    }

    fn mechanisms(
        &mut self,
        _: MechanismsParams,
        mut result: MechanismsResults,
    ) -> Promise<(), ::capnp::Error> {
        let mut builder = result.get();
        let mechs: Vec<_> = self.ctx.server_mech_list()
                                .into_iter()
                                .map(|m| m.mechanism.as_str())
                                .collect();
        let mut mechbuilder = builder.init_mechs(mechs.len() as u32);
        for (i,m) in mechs.iter().enumerate() {
            mechbuilder.set(i as u32, m);
        }

        Promise::ok(())
    }

    fn create_session(
        &mut self,
        params: CreateSessionParams,
        mut result: CreateSessionResults,
    ) -> Promise<(), ::capnp::Error> {
        let params = pry!(params.get());
        let mechanism: &str = pry!(params.get_mechanism());

        let mechname = mechanism.as_bytes();
        let state = if let Ok(mechname) = Mechname::new(mechname) {
            if let Ok(session) = self.ctx.server_start(mechname) {
                State::Running(session)
            } else {
                State::Aborted
            }
        } else {
            State::InvalidMechanism
        };

        let auth = Auth::new(self.log.clone(), self.db.clone(), state, self.nw.clone());

        let mut builder = result.get();
        builder.set_authentication(capnp_rpc::new_client(auth));

        Promise::ok(())
    }
}
