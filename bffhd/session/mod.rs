use std::path::PathBuf;
use std::sync::Arc;
use anyhow::Context;
use lmdb::Environment;
use once_cell::sync::OnceCell;
use crate::authorization::roles::Role;
use crate::resources::Resource;
use crate::session::db::SessionCache;
use crate::Users;
use crate::users::UserRef;

mod db;

static SESSION_CACHE: OnceCell<SessionCache> = OnceCell::new();

#[derive(Clone)]
pub struct SessionManager {
    users: Users,
}
impl SessionManager {
    pub fn new(users: Users) -> Self {
        Self { users }
    }

    // TODO: make infallible
    pub fn open(&self, uid: impl AsRef<str>) -> Option<SessionHandle> {
        let uid = uid.as_ref();
        if let Some(user) = self.users.get_user(uid) {
            tracing::trace!(uid, "opening new session for user");
            Some(SessionHandle { user: UserRef::new(user.id) })
        } else {
            None
        }
    }
}

#[derive(Clone, Debug)]
pub struct SessionHandle {
    user: UserRef,
}

impl SessionHandle {
    pub fn get_user(&self) -> UserRef {
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