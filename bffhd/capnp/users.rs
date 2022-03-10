use capnp::capability::Promise;
use capnp::Error;
use capnp_rpc::pry;

use api::users::Server as UsersServer;

use api::user::{
    info,
    manage,
    admin,
    passwd,
};


#[derive(Debug, Clone)]
pub struct Users {

}

impl Users {
    pub fn new() -> Self {
        Self {

        }
    }
}

impl UsersServer for Users {

}

struct User {

}

impl info::Server for User {
    fn list_roles(
        &mut self,
        _params: info::ListRolesParams,
        mut results: info::ListRolesResults
    ) -> Promise<(), Error>
    {
unimplemented!()
    }
}

impl manage::Server for User {
    fn add_role(
        &mut self,
        params: manage::AddRoleParams,
        _: manage::AddRoleResults
    ) -> Promise<(), Error> {
unimplemented!()
    }

    fn remove_role(
        &mut self,
        params: manage::RemoveRoleParams,
        _: manage::RemoveRoleResults
    ) -> Promise<(), Error> {
unimplemented!()
    }
}

impl admin::Server for User {

}

impl passwd::Server for User {

}
