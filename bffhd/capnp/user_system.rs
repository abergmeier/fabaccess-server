



use api::usersystem_capnp::user_system::{
    Server as UserSystem, info::Server as InfoServer, manage::Server as ManageServer,
};

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