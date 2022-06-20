use crate::authorization::roles::Role;
use crate::Roles;
use api::permissionsystem_capnp::permission_system::info::{
    GetRoleListParams, GetRoleListResults, Server as PermissionSystem,
};
use capnp::capability::Promise;
use capnp::Error;

use crate::session::SessionHandle;

pub struct Permissions {
    roles: Roles,
}

impl Permissions {
    pub fn new(session: SessionHandle) -> Self {
        Self {
            roles: session.roles,
        }
    }
}

impl PermissionSystem for Permissions {
    fn get_role_list(
        &mut self,
        _: GetRoleListParams,
        mut results: GetRoleListResults,
    ) -> Promise<(), Error> {
        let roles = self.roles.list().collect::<Vec<&String>>();
        let mut builder = results.get();
        let mut b = builder.init_role_list(roles.len() as u32);
        for (i, role) in roles.into_iter().enumerate() {
            let mut role_builder = b.reborrow().get(i as u32);
            role_builder.set_name(role);
        }
        Promise::ok(())
    }
}
