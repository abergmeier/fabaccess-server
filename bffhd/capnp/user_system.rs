use capnp::capability::Promise;
use capnp_rpc::pry;
use api::usersystem_capnp::user_system::{
    info, manage,
};

use crate::capnp::user::User;

use crate::session::SessionHandle;
use crate::users::db;


#[derive(Clone)]
pub struct Users {
    session: SessionHandle,
}

impl Users {
    pub fn new(session: SessionHandle) -> Self {
        Self { session }
    }
}

impl info::Server for Users {
    fn get_user_self(
        &mut self,
        _: info::GetUserSelfParams,
        mut result: info::GetUserSelfResults,
    ) -> Promise<(), ::capnp::Error> {
        let builder = result.get();
        User::build(self.session.clone(), builder);
        Promise::ok(())
    }
}

impl manage::Server for Users {
    fn get_user_list(
        &mut self,
        _: manage::GetUserListParams,
        mut result: manage::GetUserListResults,
    ) -> Promise<(), ::capnp::Error> {
        let userdb = self.session.users.into_inner();
        let users = pry!(userdb.get_all()
            .map_err(|e| capnp::Error::failed(format!("UserDB error: {:?}", e))));
        let mut builder = result.get().init_user_list(users.len() as u32);
        for (i, (_, user)) in users.into_iter().enumerate() {
            User::fill(&self.session, user, builder.reborrow().get(i as u32));
        }
        Promise::ok(())
    }
    fn add_user(
        &mut self,
        params: manage::AddUserParams,
        mut result: manage::AddUserResults,
    ) -> Promise<(), ::capnp::Error> {
        let params = pry!(params.get());
        let username = pry!(params.get_username());
        let password = pry!(params.get_password());
        // FIXME: saslprep passwords & usernames before storing them

        if !username.is_empty() && !password.is_empty() {
            if self.session.users.get_user(username).is_none() {
                let user = db::User::new_with_plain_pw(username, password);
                self.session.users.put_user(username, &user);
                let mut builder = result.get();
                User::fill(&self.session, user, builder);
            } else {
                tracing::warn!("Failed to add user: Username taken");
            }
        } else {
            if username.is_empty() {
                tracing::warn!("Failed to add user: Username empty");
            } else if password.is_empty() {
                tracing::warn!("Failed to add user: Password empty");
            }
        }

        Promise::ok(())
    }
    fn remove_user(
        &mut self,
        params: manage::RemoveUserParams,
        _: manage::RemoveUserResults,
    ) -> Promise<(), ::capnp::Error> {
        let who: &str = pry!(pry!(pry!(params.get()).get_user()).get_username());

        if let Err(e) = self.session.users.del_user(who) {
            tracing::warn!("Failed to delete user: {:?}", e);
        } else {
            tracing::info!("Deleted user {}", who);
        }

        Promise::ok(())
    }
}
