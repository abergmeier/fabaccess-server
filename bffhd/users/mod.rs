/*
 * Copyright © 2022 RLKM UG (haftungsbeschränkt).
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
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