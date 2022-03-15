



use once_cell::sync::OnceCell;
use crate::authorization::roles::{Roles};
use crate::resources::Resource;
use crate::session::db::SessionCache;
use crate::Users;
use crate::users::UserRef;

mod db;

static SESSION_CACHE: OnceCell<SessionCache> = OnceCell::new();

#[derive(Clone)]
pub struct SessionManager {
    users: Users,
    roles: Roles,

    // cache: SessionCache // todo
}
impl SessionManager {
    pub fn new(users: Users, roles: Roles) -> Self {
        Self { users, roles }
    }

    // TODO: make infallible
    pub fn open(&self, uid: impl AsRef<str>) -> Option<SessionHandle> {
        let uid = uid.as_ref();
        if let Some(user) = self.users.get_user(uid) {
            tracing::trace!(uid, "opening new session for user");
            Some(SessionHandle {
                users: self.users.clone(),
                roles: self.roles.clone(),
                user: UserRef::new(user.id),
            })
        } else {
            None
        }
    }
}

#[derive(Clone)]
pub struct SessionHandle {
    users: Users,
    roles: Roles,

    user: UserRef,
}

impl SessionHandle {
    pub fn get_user(&self) -> UserRef {
        self.user.clone()
    }

    pub fn has_disclose(&self, resource: &Resource) -> bool {
        if let Some(user) = self.users.get_user(self.user.get_username()) {
            self.roles.is_permitted(&user.userdata, &resource.get_required_privs().disclose)
        } else {
            false
        }
    }
    pub fn has_read(&self, resource: &Resource) -> bool {
        if let Some(user) = self.users.get_user(self.user.get_username()) {
            self.roles.is_permitted(&user.userdata, &resource.get_required_privs().read)
        } else {
            false
        }
    }
    pub fn has_write(&self, resource: &Resource) -> bool {
        if let Some(user) = self.users.get_user(self.user.get_username()) {
            self.roles.is_permitted(&user.userdata, &resource.get_required_privs().write)
        } else {
            false
        }
    }
    pub fn has_manage(&self, resource: &Resource) -> bool {
        if let Some(user) = self.users.get_user(self.user.get_username()) {
            self.roles.is_permitted(&user.userdata, &resource.get_required_privs().manage)
        } else {
            false
        }
    }
}