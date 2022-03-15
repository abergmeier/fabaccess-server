use crate::db::{AllocAdapter, Environment, RawDB, Result, DB};
use crate::db::{DatabaseFlags, LMDBorrow, RoTransaction, WriteFlags};
use lmdb::{RwTransaction, Transaction};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use anyhow::Context;

use rkyv::{Archived, Deserialize};

#[derive(
    Clone,
    PartialEq,
    Eq,
    Debug,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct User {
    pub id: String,
    pub userdata: UserData,
}

impl User {
    pub fn check_password(&self, pwd: &[u8]) -> anyhow::Result<bool> {
        if let Some(ref encoded) = self.userdata.passwd {
            argon2::verify_encoded(encoded, pwd)
                .context("Stored password is an invalid string")
        } else {
            Ok(false)
        }
    }
}

#[derive(
Clone,
PartialEq,
Eq,
Debug,
rkyv::Archive,
rkyv::Serialize,
rkyv::Deserialize,
serde::Serialize,
serde::Deserialize,
)]
/// Data on an user to base decisions on
///
/// This of course includes authorization data, i.e. that users set roles
pub struct UserData {
    /// A Person has N ≥ 0 roles.
    /// Persons are only ever given roles, not permissions directly
    pub roles: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub passwd: Option<String>,

    /// Additional data storage
    #[serde(flatten, skip_serializing_if = "HashMap::is_empty")]
    kv: HashMap<String, String>,
}

impl UserData {
    pub fn new(roles: Vec<String>) -> Self {
        Self { roles, kv: HashMap::new(), passwd: None }
    }
    pub fn new_with_kv(roles: Vec<String>, kv: HashMap<String, String>) -> Self {
        Self { roles, kv, passwd: None }
    }
}

type Adapter = AllocAdapter<User>;
#[derive(Clone, Debug)]
pub struct UserDB {
    env: Arc<Environment>,
    db: DB<Adapter>,
}

impl UserDB {
    pub unsafe fn new(env: Arc<Environment>, db: RawDB) -> Self {
        let db = DB::new_unchecked(db);
        Self { env, db }
    }

    pub unsafe fn open(env: Arc<Environment>) -> Result<Self> {
        let db = RawDB::open(&env, Some("user"))?;
        Ok(Self::new(env, db))
    }

    pub unsafe fn create(env: Arc<Environment>) -> Result<Self> {
        let flags = DatabaseFlags::empty();
        let db = RawDB::create(&env, Some("user"), flags)?;
        Ok(Self::new(env, db))
    }

    pub fn get(&self, uid: &str) -> Result<Option<LMDBorrow<RoTransaction, Archived<User>>>> {
        let txn = self.env.begin_ro_txn()?;
        if let Some(state) = self.db.get(&txn, &uid.as_bytes())? {
            let ptr = state.into();
            Ok(Some(unsafe { LMDBorrow::new(ptr, txn) }))
        } else {
            Ok(None)
        }
    }

    pub fn put(&self, uid: &str, user: &User) -> Result<()> {
        let mut txn = self.env.begin_rw_txn()?;
        let flags = WriteFlags::empty();
        self.db.put(&mut txn, &uid.as_bytes(), user, flags)?;
        Ok(())
    }

    pub fn get_all(&self) -> Result<Vec<(String, User)>> {
        let txn = self.env.begin_ro_txn()?;
        let mut cursor = self.db.open_ro_cursor(&txn)?;
        let iter = cursor.iter_start();
        let mut out = Vec::new();
        let mut deserializer = rkyv::Infallible;
        for user in iter {
            let (uid, user) = user?;
            let uid = unsafe { std::str::from_utf8_unchecked(uid).to_string() };
            let user: User = user.deserialize(&mut deserializer).unwrap();
            out.push((uid, user));
        }

        Ok(out)
    }
}