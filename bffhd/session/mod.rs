use std::sync::Arc;
use crate::authorization::roles::Role;
use crate::resources::Resource;
use crate::users::User;

struct Inner {

}
impl Inner {
    pub fn new() -> Self {
        Self { }
    }
}

#[derive(Clone)]
pub struct SessionManager {
    inner: Arc<Inner>,
}
impl SessionManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner::new()),
        }
    }
    pub fn open(&self, uid: impl AsRef<str>) -> Option<SessionHandle> {
        unimplemented!()
    }
}

#[derive(Clone, Debug)]
pub struct SessionHandle {
}

impl SessionHandle {
    pub fn get_user(&self) -> User {
        unimplemented!()
    }

    pub fn has_disclose(&self, resource: &Resource) -> bool {
        unimplemented!()
    }
    pub fn has_read(&self, resource: &Resource) -> bool {
        unimplemented!()
    }
    pub fn has_write(&self, resource: &Resource) -> bool {
        unimplemented!()
    }
    pub fn has_manage(&self, resource: &Resource) -> bool {
        unimplemented!()
    }
}