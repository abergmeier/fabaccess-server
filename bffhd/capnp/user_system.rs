use capnp::capability::Promise;
use api::usersystem_capnp::user_system::{
    info, manage, Server as UserSystem,
    self as system,
};
use crate::authorization::permissions::Permission;
use crate::capnp::user::User;

use crate::session::SessionHandle;

#[derive(Clone)]
pub struct Users {
    session: SessionHandle,
}

impl Users {
    pub fn new(session: SessionHandle) -> Self {
        Self { session }
    }
}

impl system::Server for Users {
    fn info(
        &mut self,
        _: system::InfoParams,
        mut result: system::InfoResults,
    ) -> Promise<(), ::capnp::Error> {
        result.get().set_info(capnp_rpc::new_client(self.clone()));
        Promise::ok(())
    }
    fn manage(
        &mut self,
        _: system::ManageParams,
        mut result: system::ManageResults,
    ) -> Promise<(), ::capnp::Error> {
        if self.session.has_perm(Permission::new("bffh.users.manage")) {
            result.get().set_manage(capnp_rpc::new_client(self.clone()));
        }
        Promise::ok(())
    }
}

impl info::Server for Users {
    fn get_user_self(
        &mut self,
        _: info::GetUserSelfParams,
        mut result: info::GetUserSelfResults,
    ) -> Promise<(), ::capnp::Error> {
        let builder = result.get().init_user();
        User::build(self.session.clone(), builder);
        Promise::ok(())
    }
}

impl manage::Server for Users {
    fn get_user_list(
        &mut self,
        _: manage::GetUserListParams,
        _: manage::GetUserListResults,
    ) -> Promise<(), ::capnp::Error> {
        Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
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
