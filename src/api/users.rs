use capnp::capability::Promise;

use crate::db::access::{PermRule, Permission};
use crate::schema::usersystem_capnp::user_system;
use crate::schema::usersystem_capnp::user_system::{info, manage};

#[derive(Clone, Debug)]
pub struct Users {
    perms: Vec<PermRule>,
}

impl Users {
    pub fn new(perms: Vec<PermRule>) -> Self {
        Self { perms }
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
        if self.perms.iter().any(|rule| rule.match_perm(perm)) {
            results
                .get()
                .set_manage(capnp_rpc::new_client(self.clone()));
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
        Promise::ok(())
    }
}

impl manage::Server for Users {}
