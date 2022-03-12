use api::permissionsystem_capnp::permission_system::Server as PermissionSystem;
use crate::authorization::AuthorizationHandle;
use crate::session::SessionHandle;

pub struct Permissions;

impl Permissions {
    pub fn new(session: SessionHandle) -> Self {
        Self
    }
}

impl PermissionSystem for Permissions {

}