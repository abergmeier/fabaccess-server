use capnp::capability::Promise;
use capnp::Error;
use capnp_rpc::pry;

use api::usersystem_capnp::user_system::{
    Server as UserSystem,
    info, info::Server as InfoServer,
    manage, manage::Server as ManageServer,
};
use crate::authorization::AuthorizationHandle;
use crate::session::SessionHandle;

#[derive(Clone)]
pub struct Users {
    session: SessionHandle,
}

impl Users {
    pub fn new(session: SessionHandle) -> Self {
        Self {
            session,
        }
    }
}

impl UserSystem for Users {

}

impl InfoServer for Users {

}

impl ManageServer for Users {

}