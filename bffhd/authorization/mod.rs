use std::sync::Arc;
use crate::authorization::permissions::Permission;
use crate::authorization::roles::{Role, Roles};
use crate::Users;
use crate::users::UserRef;

pub mod permissions;
pub mod roles;

#[derive(Clone)]
pub struct AuthorizationHandle {
    users: Users,
    roles: Roles,
}

impl AuthorizationHandle {
    pub fn new(users: Users, roles: Roles) -> Self {
        Self { users, roles }
    }

    pub fn get_user_roles(&self, uid: impl AsRef<str>) -> Option<Vec<String>> {
        let user = self.users.get_user(uid.as_ref())?;
        Some(user.userdata.roles.clone())
    }
}