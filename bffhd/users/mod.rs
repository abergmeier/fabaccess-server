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

use std::ops::Deref;
use std::sync::Arc;
use rkyv::{Archive, Serialize, Deserialize, Infallible};

mod db;

pub use db::UserDB;
pub use crate::authentication::db::PassDB;

#[derive(Debug, Clone, Archive, Serialize, Deserialize, serde::Serialize, serde::Deserialize)]
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