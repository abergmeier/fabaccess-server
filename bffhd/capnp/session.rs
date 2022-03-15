use api::authenticationsystem_capnp::response::successful::Builder;


use crate::capnp::machinesystem::Machines;
use crate::capnp::permissionsystem::Permissions;
use crate::capnp::user_system::Users;
use crate::session::{SessionHandle};


#[derive(Debug, Clone)]
pub struct APISession;

impl APISession {
    pub fn new() -> Self {
        Self
    }

    pub fn build(session: SessionHandle, builder: Builder) {
        let mut builder = builder.init_session();
        builder.set_machine_system(capnp_rpc::new_client(Machines::new(session.clone())));
        builder.set_user_system(capnp_rpc::new_client(Users::new(session.clone())));
        builder.set_permission_system(capnp_rpc::new_client(Permissions::new(session)));
    }
}