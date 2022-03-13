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

use rkyv::{Archive, Deserialize, Infallible, Serialize};
use std::ops::Deref;
use std::sync::Arc;

pub mod db;

pub use crate::authentication::db::PassDB;
use crate::authorization::roles::Role;

#[derive(
    Copy,
    Clone,
    PartialEq,
    Eq,
    Debug,
    Archive,
    Serialize,
    Deserialize,
    serde::Serialize,
    serde::Deserialize,
)]
#[archive_attr(derive(Debug, PartialEq, serde::Serialize, serde::Deserialize))]
pub struct User {
    id: u64
}

impl User {
    pub fn new(id: u64) -> Self {
        User { id }
    }

    pub fn get_username(&self) -> &str {
        unimplemented!()
    }

    pub fn get_roles(&self) -> impl IntoIterator<Item=Role> {
        unimplemented!();
        []
    }
}
