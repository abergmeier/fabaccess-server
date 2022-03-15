use std::sync::Arc;
use crate::authorization::permissions::Permission;
use crate::authorization::roles::Role;
use crate::Users;
use crate::users::User;

pub mod permissions;
pub mod roles;

struct Inner {
    users: Users,
}
impl Inner {
    pub fn new(users: Users) -> Self {
       Self { users }
    }
}

#[derive(Clone)]
pub struct AuthorizationHandle {
    users: Users,
}

impl AuthorizationHandle {
    pub fn new(users: Users) -> Self {
        Self { users }
    }

    pub fn get_user_roles(&self, uid: impl AsRef<str>) -> Option<impl IntoIterator<Item=Role>> {
        let user = self.users.get_user(uid.as_ref())?;
        Some([])
    }

    pub fn is_permitted<'a>(&self, roles: impl IntoIterator<Item=&'a Role>, perm: impl AsRef<Permission>) -> bool {
        unimplemented!()
    }
}