use crate::authorization::permissions::Permission;
use crate::authorization::roles::Roles;
use crate::resources::Resource;
use crate::users::{db, UserRef};
use crate::Users;
use tracing::Span;
use crate::users::db::User;

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

    pub fn try_open(&self, parent: &Span, uid: impl AsRef<str>) -> Option<SessionHandle> {
        self.users.get_user(uid.as_ref()).map(|user| self.open(parent, user))
    }

    // TODO: make infallible
    pub fn open(&self, parent: &Span, user: User) -> SessionHandle {
        let uid = user.id.as_str();
        let span = tracing::info_span!(
            target: "bffh::api",
            parent: parent,
            "session",
            uid,
        );
        tracing::trace!(parent: &span, uid, ?user, "opening session");
        SessionHandle {
            span,
            users: self.users.clone(),
            roles: self.roles.clone(),
            user: UserRef::new(user.id),
        }
    }
}

#[derive(Clone)]
pub struct SessionHandle {
    pub span: Span,

    pub users: Users,
    pub roles: Roles,

    user: UserRef,
}

impl SessionHandle {
    pub fn get_user_ref(&self) -> UserRef {
        self.user.clone()
    }

    pub fn get_user(&self) -> db::User {
        self.users
            .get_user(self.user.get_username())
            .expect("Failed to get user self")
    }

    pub fn has_disclose(&self, resource: &Resource) -> bool {
        if let Some(user) = self.users.get_user(self.user.get_username()) {
            self.roles
                .is_permitted(&user.userdata, &resource.get_required_privs().disclose)
        } else {
            false
        }
    }
    pub fn has_read(&self, resource: &Resource) -> bool {
        if let Some(user) = self.users.get_user(self.user.get_username()) {
            self.roles
                .is_permitted(&user.userdata, &resource.get_required_privs().read)
        } else {
            false
        }
    }
    pub fn has_write(&self, resource: &Resource) -> bool {
        if let Some(user) = self.users.get_user(self.user.get_username()) {
            self.roles
                .is_permitted(&user.userdata, &resource.get_required_privs().write)
        } else {
            false
        }
    }
    pub fn has_manage(&self, resource: &Resource) -> bool {
        if let Some(user) = self.users.get_user(self.user.get_username()) {
            self.roles
                .is_permitted(&user.userdata, &resource.get_required_privs().manage)
        } else {
            false
        }
    }
    pub fn has_perm(&self, perm: impl AsRef<Permission>) -> bool {
        if let Some(user) = self.users.get_user(self.user.get_username()) {
            self.roles.is_permitted(&user.userdata, perm)
        } else {
            false
        }
    }
}
