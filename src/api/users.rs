use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::ops::Deref;

use capnp::capability::Promise;

use crate::api::user::User;
use crate::connection::Session;
use crate::db::access::{PermRule, Permission};
use crate::db::user::{UserId, Internal as UserDB};
use crate::schema::usersystem_capnp::user_system;
use crate::schema::usersystem_capnp::user_system::{info, manage};
use crate::error;

#[derive(Clone, Debug)]
pub struct Users {
    session: Rc<RefCell<Option<Session>>>,
    userdb: Arc<UserDB>,
}

impl Users {
    pub fn new(session: Rc<RefCell<Option<Session>>>, userdb: Arc<UserDB>) -> Self {
        Self { session, userdb }
    }
}

impl user_system::Server for Users {
    fn info(
        &mut self,
        _: user_system::InfoParams,
        mut results: user_system::InfoResults,
    ) -> Promise<(), capnp::Error> {
        results.get().set_info(capnp_rpc::new_client(self.clone()));
        Promise::ok(())
    }

    fn manage(
        &mut self,
        _: user_system::ManageParams,
        mut results: user_system::ManageResults,
    ) -> Promise<(), capnp::Error> {
        let perm: &Permission = Permission::new("bffh.users.manage");
        if let Some(session) = self.session.borrow().deref() {
            if session.perms.iter().any(|rule| rule.match_perm(perm)) {
                results
                    .get()
                    .set_manage(capnp_rpc::new_client(self.clone()));
            }
        }

        Promise::ok(())
    }
}

impl info::Server for Users {
    fn get_user_self(
        &mut self,
        _: info::GetUserSelfParams,
        mut results: info::GetUserSelfResults,
    ) -> Promise<(), capnp::Error> {
        let user = User::new(Rc::clone(&self.session));
        let mut builder = results.get().init_user();
        user.fill_self(&mut builder);
        Promise::ok(())
    }
}

impl manage::Server for Users {
    fn get_user_list(
        &mut self,
        _: manage::GetUserListParams,
        mut results: manage::GetUserListResults,
    ) -> Promise<(), capnp::Error> {
        let result: Result<(), error::Error> = 
            self.userdb.list_users()
                .and_then(|users| {
                    let builder = results.get().init_user_list(users.len() as u32);
                    let u = User::new(Rc::clone(&self.session));
                    for (i, user) in users.into_iter().enumerate() {
                        let mut b = builder.reborrow().get(i as u32);
                        u.fill_with(&mut b, user);
                    }
                    Ok(())
                });

        match result {
            Ok(()) => Promise::ok(()),
            Err(e) => Promise::err(capnp::Error::failed("User lookup failed: {}".to_string())),
        }
    }

    /*fn add_user(
        &mut self,
        params: manage::AddUserParams,
        mut results: manage::AddUserResults,
    ) -> Promise<(), capnp::Error> {
    }

    fn remove_user(
        &mut self,
        _: manage::RemoveUserParams,
        mut results: manage::RemoveUserResults,
    ) -> Promise<(), capnp::Error> {
    }*/
}
