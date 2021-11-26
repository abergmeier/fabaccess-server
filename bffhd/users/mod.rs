use rkyv::{Archive, Serialize, Deserialize};

use capnp::capability::Promise;
use capnp::Error;

use api::user::{
    info,
    manage,
    admin,
};

mod db;
pub use db::UserDB;

#[derive(Debug, Clone, Archive, Serialize, Deserialize, serde::Serialize, serde::Deserialize)]
pub struct User {
    id: u128,
    username: String,
    roles: Vec<String>,
}

impl User {

}

impl info::Server for User {
    fn list_roles(
        &mut self,
        params: info::ListRolesParams,
        mut results: info::ListRolesResults
    ) -> Promise<(), Error>
    {
        Promise::ok(())
    }
}

impl manage::Server for User {

}

impl admin::Server for User {

}