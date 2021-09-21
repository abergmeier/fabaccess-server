//! UserDB does two kinds of lookups:
//! 1. "I have this here username, what user is that"
//! 2. "I have this here user, what are their roles (and other associated data)"
use serde::{Serialize, Deserialize};
use std::fmt;
use std::fs;
use std::sync::Arc;
use std::iter::FromIterator;
use std::path::Path;
use crate::db::access::RoleIdentifier;
use std::collections::HashMap;

use slog::Logger;

use crate::error::Result;
use crate::config::Config;

mod internal;
pub use internal::Internal;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// An user
pub struct User {
    /// The precise (and unique) identifier of this user
    pub id: UserId,
    /// Data BFFH stores on this user to base decisions on
    pub data: UserData,
}

impl User {
    pub fn new(id: UserId, data: UserData) -> Self {
        Self { id, data }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
/// Authorization Identity
///
/// This identity is internal to FabAccess and completely independent from the authentication
/// method or source
pub struct UserId {
    /// Main User ID. Generally an user name or similar. Locally unique
    pub uid: String,
    /// Sub user ID. 
    ///
    /// Can change scopes for permissions, e.g. having a +admin account with more permissions than
    /// the default account and +dashboard et.al. accounts that have restricted permissions for
    /// their applications
    pub subuid: Option<String>,
    /// Realm this account originates.
    ///
    /// The Realm is usually described by a domain name but local policy may dictate an unrelated
    /// mapping
    pub realm: Option<String>,
}

impl UserId {
    pub fn new(uid: String, subuid: Option<String>, realm: Option<String>) -> Self {
        Self { uid, subuid, realm }
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = write!(f, "{}", self.uid);
        if let Some(ref s) = self.subuid {
            write!(f, "+{}", s)?;
        }
        if let Some(ref l) = self.realm {
            write!(f, "@{}", l)?;
        }
        r
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
/// Data on an user to base decisions on
///
/// This of course includes authorization data, i.e. that users set roles
pub struct UserData {
    /// A Person has N ≥ 0 roles.
    /// Persons are only ever given roles, not permissions directly
    pub roles: Vec<RoleIdentifier>,

    #[serde(skip_serializing_if = "is_zero")]
    #[serde(default = "default_priority")]
    /// A priority number, defaulting to 0.
    ///
    /// The higher, the higher the priority. Higher priority users overwrite lower priority ones.
    pub priority: u64,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub passwd: Option<String>,

    /// Additional data storage
    #[serde(flatten, skip_serializing_if = "HashMap::is_empty")]
    kv: HashMap<String, String>,
}

impl UserData {
    pub fn new(roles: Vec<RoleIdentifier>, priority: u64) -> Self {
        Self { roles, priority, kv: HashMap::new(), passwd: None }
    }
}

fn is_zero(i: &u64) -> bool {
    *i == 0
}
const fn default_priority() -> u64 {
    0
}

pub fn load_file<P: AsRef<Path>>(path: P) -> Result<HashMap<String, User>> {
    let f = fs::read(path)?;
    let mut map: HashMap<String, UserData> = toml::from_slice(&f)?;

    Ok(HashMap::from_iter(map.drain().map(|(uid, mut user_data)| {
        user_data.passwd = user_data.passwd.map(|pw| if !pw.starts_with("$argon2") {
            let config = argon2::Config::default();
            let salt: [u8; 16] = rand::random();
            let hash = argon2::hash_encoded(pw.as_bytes(), &salt, &config)
                .expect(&format!("Failed to hash password for {}: ", uid));
            println!("Hashed pw for {} to {}", uid, hash);

            hash
        } else {
            pw
        });
        ( uid.clone()
        , User::new(UserId::new(uid, None, None), user_data)
        )
    })))
}

pub fn init(log: Logger, _config: &Config, env: Arc<lmdb::Environment>) -> Result<Internal> {
    let mut flags = lmdb::DatabaseFlags::empty();
    let db = env.create_db(Some("userdb"), flags)?;
    debug!(&log, "Opened user db successfully.");

    Ok(Internal::new(log, env, db))
}
