use crate::authorization::permissions::Permission;
use api::authenticationsystem_capnp::response::successful::Builder;

use crate::capnp::machinesystem::Machines;
use crate::capnp::permissionsystem::Permissions;
use crate::capnp::user_system::Users;
use crate::session::SessionHandle;

#[derive(Debug, Clone)]
pub struct APISession;

impl APISession {
    pub fn new() -> Self {
        Self
    }

    pub fn build(session: SessionHandle, builder: Builder) {
        let mut builder = builder.init_session();

        {
            let mut b = builder.reborrow().init_machine_system();
            b.set_info(capnp_rpc::new_client(Machines::new(session.clone())));
        }

        {
            let mut b = builder.reborrow().init_user_system();
            let u = Users::new(session.clone());
            if session.has_perm(Permission::new("bffh.users.manage")) {
                b.set_manage(capnp_rpc::new_client(u.clone()));
                b.set_search(capnp_rpc::new_client(u.clone()));
            }
            b.set_info(capnp_rpc::new_client(u));
        }

        {
            let mut b = builder.init_permission_system();
            b.set_info(capnp_rpc::new_client(Permissions::new(session)));
        }
    }
}
