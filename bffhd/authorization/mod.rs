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

    pub fn lookup_user(&self, uid: impl AsRef<str>) -> Option<User> {
        unimplemented!()
    }

    pub fn is_permitted<'a>(&self, roles: impl IntoIterator<Item=&'a Role>, perm: impl AsRef<Permission>) -> bool {
        unimplemented!()
    }
}