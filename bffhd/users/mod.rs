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

use anyhow::Context;
use lmdb::Environment;
use once_cell::sync::OnceCell;
use rkyv::{Archive, Deserialize, Infallible, Serialize};
use std::collections::HashMap;
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;

pub mod db;



use crate::users::db::UserData;
use crate::UserDB;

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
pub struct UserRef {
    id: String,
}

impl UserRef {
    pub fn new(id: String) -> Self {
        UserRef { id }
    }

    pub fn get_username(&self) -> &str {
        self.id.as_str()
    }
}

static USERDB: OnceCell<UserDB> = OnceCell::new();

#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct Users {
    userdb: &'static UserDB,
}

impl Users {
    pub fn new(env: Arc<Environment>) -> anyhow::Result<Self> {
        let span = tracing::debug_span!("users", ?env, "Creating Users handle");
        let _guard = span.enter();

        let userdb = USERDB
            .get_or_try_init(|| {
                tracing::debug!("Global resource not yet initialized, initializing…");
                unsafe { UserDB::create(env) }
            })
            .context("Failed to open userdb")?;

        Ok(Self { userdb })
    }

    pub(crate) fn into_inner(self) -> &'static UserDB {
        self.userdb
    }

    pub fn get_user(&self, uid: &str) -> Option<db::User> {
        tracing::trace!(uid, "Looking up user");
        self.userdb
            .get(uid)
            .unwrap()
            .map(|user| Deserialize::<db::User, _>::deserialize(user.deref(), &mut Infallible).unwrap())
    }

    pub fn load_file<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<()> {
        let f = std::fs::read(path)?;
        let map: HashMap<String, UserData> = toml::from_slice(&f)?;

        for (uid, mut userdata) in map {
            userdata.passwd = userdata.passwd.map(|pw| {
                if !pw.starts_with("$argon2") {
                    let config = argon2::Config::default();
                    let salt: [u8; 16] = rand::random();
                    let hash = argon2::hash_encoded(pw.as_bytes(), &salt, &config)
                        .expect(&format!("Failed to hash password for {}: ", uid));
                    tracing::debug!("Hashed pw for {} to {}", uid, hash);

                    hash
                } else {
                    pw
                }
            });
            let user = db::User {
                id: uid.clone(),
                userdata,
            };
            tracing::trace!(%uid, ?user, "Storing user object");
            self.userdb.put(uid.as_str(), &user);
        }

        Ok(())
    }
}
