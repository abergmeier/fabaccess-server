use api::usersystem_capnp::user_system::{info, manage, search};
use capnp::capability::Promise;
use capnp_rpc::pry;
use tracing::Span;

use crate::capnp::user::User;

use crate::session::SessionHandle;
use crate::users::{db, UserRef};

const TARGET: &str = "bffh::api::usersystem";

#[derive(Clone)]
pub struct Users {
    span: Span,
    session: SessionHandle,
}

impl Users {
    pub fn new(session: SessionHandle) -> Self {
        let span = tracing::info_span!(target: TARGET, "UserSystem",);
        Self { span, session }
    }
}

impl info::Server for Users {
    fn get_user_self(
        &mut self,
        _: info::GetUserSelfParams,
        mut result: info::GetUserSelfResults,
    ) -> Promise<(), ::capnp::Error> {
        let _guard = self.span.enter();
        let _span = tracing::trace_span!(target: TARGET, "getUserSelf").entered();
        tracing::trace!("method call");

        let builder = result.get();
        User::build(self.session.clone(), builder);

        tracing::trace!("method return");
        Promise::ok(())
    }
}

impl manage::Server for Users {
    fn get_user_list(
        &mut self,
        _: manage::GetUserListParams,
        mut result: manage::GetUserListResults,
    ) -> Promise<(), ::capnp::Error> {
        let _guard = self.span.enter();
        let _span = tracing::trace_span!(target: TARGET, "getUserList",).entered();
        tracing::trace!("method call");

        let userdb = self.session.users.into_inner();
        let users = pry!(userdb
            .get_all()
            .map_err(|e| capnp::Error::failed(format!("UserDB error: {:?}", e))));
        let mut builder = result.get().init_user_list(users.len() as u32);
        for (i, (id, userdata)) in users.into_iter().enumerate() {
            let user = db::User { id, userdata };
            User::fill(&self.session, user, builder.reborrow().get(i as u32));
        }

        tracing::trace!("method return");
        Promise::ok(())
    }

    fn add_user_fallible(
        &mut self,
        params: manage::AddUserFallibleParams,
        mut result: manage::AddUserFallibleResults,
    ) -> Promise<(), ::capnp::Error> {
        let _guard = self.span.enter();
        let _span = tracing::trace_span!(target: TARGET, "addUserFallible").entered();

        let params = pry!(params.get());
        let username = pry!(params.get_username());
        let password = pry!(params.get_password());
        // FIXME: saslprep passwords & usernames before storing them

        tracing::trace!(
            params.username = username,
            params.password = "<redacted>",
            "method call"
        );

        let builder = result.get();

        if !username.is_empty() && !password.is_empty() {
            if self.session.users.get_user(username).is_none() {
                let user = db::User::new_with_plain_pw(username, password);
                pry!(self.session.users.put_user(username, &user));
                let builder = builder.init_successful();
                User::fill(&self.session, user, builder);
            } else {
                let mut builder = builder.init_failed();
                builder.set_error(manage::add_user_error::AddUserError::AlreadyExists);
                tracing::warn!("Failed to add user: Username taken");
            }
        } else {
            if username.is_empty() {
                let mut builder = builder.init_failed();
                builder.set_error(manage::add_user_error::AddUserError::UsernameInvalid);
                tracing::warn!("Failed to add user: Username empty");
            } else if password.is_empty() {
                let mut builder = builder.init_failed();
                builder.set_error(manage::add_user_error::AddUserError::PasswordInvalid);
                tracing::warn!("Failed to add user: Password empty");
            }
        }

        tracing::trace!("method return");
        Promise::ok(())
    }

    fn remove_user(
        &mut self,
        params: manage::RemoveUserParams,
        _: manage::RemoveUserResults,
    ) -> Promise<(), ::capnp::Error> {
        let _guard = self.span.enter();
        let _span = tracing::trace_span!(target: TARGET, "removeUser",).entered();

        let who: &str = pry!(pry!(pry!(params.get()).get_user()).get_username());

        tracing::trace!(params.user = who, "method call");

        if let Err(e) = self.session.users.del_user(who) {
            tracing::warn!("Failed to delete user: {:?}", e);
        } else {
            tracing::info!("Deleted user {}", who);
        }

        tracing::trace!("method return");
        Promise::ok(())
    }
}

impl search::Server for Users {
    fn get_user_by_name(
        &mut self,
        params: search::GetUserByNameParams,
        mut result: search::GetUserByNameResults,
    ) -> Promise<(), ::capnp::Error> {
        let _guard = self.span.enter();
        let _span = tracing::trace_span!(target: TARGET, "getUserByName",).entered();

        let username: &str = pry!(pry!(params.get()).get_username());

        tracing::trace!(params.username = username, "method call");

        let userref = UserRef::new(username.to_string());
        User::build_optional(&self.session, Some(userref), result.get());

        tracing::trace!("method return");
        Promise::ok(())
    }
}
