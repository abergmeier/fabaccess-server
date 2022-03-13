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

use std::collections::HashMap;
use rkyv::{Archive, Deserialize, Infallible, Serialize};
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;
use anyhow::Context;
use lmdb::Environment;

pub mod db;

pub use crate::authentication::db::PassDB;
use crate::authorization::roles::{Role, RoleIdentifier};
use crate::UserDB;
use crate::users::db::UserData;

#[derive(
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
#[archive_attr(derive(Debug, PartialEq))]
pub struct User {
    id: String,
}

impl User {
    pub fn new(id: String) -> Self {
        User { id }
    }

    pub fn get_username(&self) -> &str {
        self.id.as_str()
    }

    pub fn get_roles(&self) -> impl IntoIterator<Item=Role> {
        unimplemented!();
        []
    }
}

pub struct Inner {
    userdb: UserDB,
    //passdb: PassDB,
}

#[derive(Clone)]
pub struct Users {
    inner: Arc<Inner>
}

impl Users {
    pub fn new(env: Arc<Environment>) -> anyhow::Result<Self> {
        let userdb = unsafe { UserDB::create(env.clone()).unwrap() };
        //let passdb = unsafe { PassDB::create(env).unwrap() };
        Ok(Self { inner: Arc::new(Inner { userdb }) })
    }

    pub fn load_file<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<()> {
        let f = std::fs::read(path)?;
        let mut map: HashMap<String, UserData> = toml::from_slice(&f)?;

        for (uid, mut userdata) in map {
            userdata.passwd = userdata.passwd.map(|pw| if !pw.starts_with("$argon2") {
                let config = argon2::Config::default();
                let salt: [u8; 16] = rand::random();
                let hash = argon2::hash_encoded(pw.as_bytes(), &salt, &config)
                    .expect(&format!("Failed to hash password for {}: ", uid));
                tracing::debug!("Hashed pw for {} to {}", uid, hash);

                hash
            } else {
                pw
            });
            let user = db::User { id: uid.clone(), userdata };
            tracing::trace!(%uid, ?user, "Storing user object");
            self.inner.userdb.put(uid.as_str(), &user);
        }

        Ok(())
    }
}