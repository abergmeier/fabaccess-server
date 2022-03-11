use capnp::capability::Promise;
use capnp::Error;
use capnp_rpc::pry;

use api::usersystem_capnp::user_system::{
    Server as UserSystem,
    info, info::Server as InfoServer,
    manage, manage::Server as ManageServer,
};

#[derive(Debug, Clone)]
pub struct Users {

}

impl Users {
    pub fn new() -> Self {
        Self {

        }
    }
}

impl UserSystem for Users {

}

impl InfoServer for Users {

}

impl ManageServer for Users {

}