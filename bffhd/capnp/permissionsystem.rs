use api::permissionsystem_capnp::permission_system::Server as PermissionSystem;

use crate::session::SessionHandle;

pub struct Permissions;

impl Permissions {
    pub fn new(_session: SessionHandle) -> Self {
        Self
    }
}

impl PermissionSystem for Permissions {

}