/*
 * Copyright (c) 2021. Lorem ipsum dolor sit amet, consectetur adipiscing elit.
 * Morbi non lorem porttitor neque feugiat blandit. Ut vitae ipsum eget quam lacinia accumsan.
 * Etiam sed turpis ac ipsum condimentum fringilla. Maecenas magna.
 * Proin dapibus sapien vel ante. Aliquam erat volutpat. Pellentesque sagittis ligula eget metus.
 * Vestibulum commodo. Ut rhoncus gravida arcu.
 */

use rkyv::{Archive, Serialize, Deserialize};

use capnp::capability::Promise;
use capnp::Error;
use capnp_rpc::pry;

use api::user::{
    info,
    manage,
    admin,
    passwd,
};

mod db;
mod pass;

pub use db::UserDB;
pub use pass::PassDB;

#[derive(Debug, Clone, Archive, Serialize, Deserialize, serde::Serialize, serde::Deserialize)]
/// User API endpoint
pub struct User {
    id: u128,
    username: String,
    roles: Vec<String>,
}

impl User {
    pub fn new(id: u128, username: String, roles: Vec<String>) -> Self {
        User { id, username, roles }
    }
}

impl info::Server for User {
    fn list_roles(
        &mut self,
        _params: info::ListRolesParams,
        mut results: info::ListRolesResults
    ) -> Promise<(), Error>
    {
        let results = results.get();
        let mut roles = results.init_roles(self.roles.len() as u32);

        for (i, role) in self.roles.iter().enumerate() {
            let mut role_builder = roles.reborrow().get(i as u32);
            role_builder.set_name(role);
        }

        Promise::ok(())
    }
}

impl manage::Server for User {
    fn add_role(
        &mut self,
        params: manage::AddRoleParams,
        _: manage::AddRoleResults
    ) -> Promise<(), Error> {
        let params = pry!(params.get());
        let name = pry!(params.get_name()).to_string();
        self.roles.push(name);
        Promise::ok(())
    }

    fn remove_role(
        &mut self,
        params: manage::RemoveRoleParams,
        _: manage::RemoveRoleResults
    ) -> Promise<(), Error> {
        let params = pry!(params.get());
        let name = pry!(params.get_name());
        self.roles.retain(|role| role != name);
        Promise::ok(())
    }
}

impl admin::Server for User {

}

impl passwd::Server for User {

}