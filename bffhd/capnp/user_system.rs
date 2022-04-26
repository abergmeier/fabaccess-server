use capnp::capability::Promise;
use capnp_rpc::pry;
use api::usersystem_capnp::user_system::{
    info, manage,
    self as system,
};
use crate::authorization::permissions::Permission;
use crate::capnp::user::User;

use crate::session::SessionHandle;
use crate::users::UserRef;

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
        let me = User::new_self(self.session.clone());
        for (i, (_, user)) in users.into_iter().enumerate() {
            me.fill(user, builder.reborrow().get(i as u32));
        }
        Promise::ok(())
    }
    fn add_user(
        &mut self,
        _: manage::AddUserParams,
        _: manage::AddUserResults,
    ) -> Promise<(), ::capnp::Error> {
        Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
    fn remove_user(
        &mut self,
        _: manage::RemoveUserParams,
        _: manage::RemoveUserResults,
    ) -> Promise<(), ::capnp::Error> {
        Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
}
