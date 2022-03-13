use std::sync::Arc;
use crate::authorization::permissions::Permission;
use crate::authorization::roles::Role;
use crate::users::User;

pub mod permissions;
pub mod roles;

struct Inner {

}
impl Inner {
    pub fn new() -> Self {
       Self {}
    }
}

#[derive(Clone)]
pub struct AuthorizationHandle {
    inner: Arc<Inner>,
}

impl AuthorizationHandle {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner::new())
        }
    }

    pub fn get_user_roles(&self, uid: impl AsRef<str>) -> Option<impl IntoIterator<Item=Role>> {
        unimplemented!();
        Some([])
    }

    pub fn is_permitted<'a>(&self, roles: impl IntoIterator<Item=&'a Role>, perm: impl AsRef<Permission>) -> bool {
        unimplemented!()
    }
}