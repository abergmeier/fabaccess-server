use crate::db::access::PermRule;
use crate::db::user as db;
use crate::schema::user_capnp::user::*;

#[derive(Clone)]
pub struct User {
    user: db::User,
    perms: Vec<PermRule>,
}

impl User {
    pub fn new(user: db::User, perms: Vec<PermRule>) -> Self {
        Self { user, perms }
    }

    pub fn fill(&self, builder: &mut Builder) {
        builder.set_username(&self.user.id.uid);
        if let Some(ref realm) = &self.user.id.realm {
            let mut space = builder.reborrow().init_space();
            space.set_name(&realm);
        }
    }
}

impl info::Server for User {}
impl manage::Server for User {}
impl admin::Server for User {}
