use std::rc::Rc;
use std::cell::RefCell;
use std::ops::Deref;

use crate::connection::Session;
use crate::db::user as db;
use crate::schema::user_capnp::user::*;

#[derive(Clone)]
pub struct User {
    session: Rc<RefCell<Option<Session>>>,
}

impl User {
    pub fn new(session: Rc<RefCell<Option<Session>>>) -> Self {
        Self { session }
    }

    pub fn fill_self(&self, builder: &mut Builder) {
        if let Some(session) = self.session.borrow().deref() {
            self.fill_userid(builder, &session.authzid);
        }
    }

    pub fn fill_with(&self, builder: &mut Builder, user: db::User) {
        self.fill_userid(builder, &user.id)
    }

    pub fn fill_userid(&self, builder: &mut Builder, uid: &db::UserId) {
        builder.set_username(&uid.uid);
        if let Some(ref realm) = &uid.realm {
            let mut space = builder.reborrow().init_space();
            space.set_name(&realm);
        }
    }
}

impl info::Server for User {

}
impl manage::Server for User {}
impl admin::Server for User {}
