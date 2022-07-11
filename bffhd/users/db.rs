use lmdb::{DatabaseFlags, Environment, RwTransaction, Transaction, WriteFlags};
use rkyv::Infallible;
use std::collections::HashMap;

use miette::{Context, IntoDiagnostic};
use std::sync::Arc;

use crate::db;
use crate::db::{AlignedAdapter, ArchivedValue, RawDB, DB};
use rkyv::ser::serializers::AllocSerializer;
use rkyv::ser::Serializer;
use rkyv::Deserialize;

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

fn hash_pw(pw: &[u8]) -> argon2::Result<String> {
    let config = argon2::Config::default();
    let salt: [u8; 16] = rand::random();
    argon2::hash_encoded(pw, &salt, &config)
}

impl User {
    pub fn check_password(&self, pwd: &[u8]) -> miette::Result<bool> {
        if let Some(ref encoded) = self.userdata.passwd {
            argon2::verify_encoded(encoded, pwd)
                .into_diagnostic()
                .wrap_err("Stored password is an invalid string")
        } else {
            Ok(false)
        }
    }

    pub fn new_with_plain_pw(username: &str, password: impl AsRef<[u8]>) -> Self {
        let hash = hash_pw(password.as_ref())
            .expect(&format!("Failed to hash password for {}: ", username));
        tracing::debug!("Hashed pw for {} to {}", username, hash);

        User {
            id: username.to_string(),
            userdata: UserData {
                passwd: Some(hash),
                ..Default::default()
            },
        }
    }

    pub fn set_pw(&mut self, password: impl AsRef<[u8]>) {
        self.userdata.passwd = Some(hash_pw(password.as_ref()).expect(&format!(
            "failed to update hashed password for {}",
            &self.id
        )));
    }
}

#[derive(
    Clone,
    PartialEq,
    Eq,
    Debug,
    Default,
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
    pub kv: HashMap<String, String>,
}

impl UserData {
    pub fn new(roles: Vec<String>) -> Self {
        Self {
            roles,
            kv: HashMap::new(),
            passwd: None,
        }
    }
    pub fn new_with_kv(roles: Vec<String>, kv: HashMap<String, String>) -> Self {
        Self {
            roles,
            kv,
            passwd: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct UserDB {
    env: Arc<Environment>,
    db: DB<AlignedAdapter<User>>,
}

impl UserDB {
    // TODO: Make an userdb-specific Transaction newtype to make this safe
    pub unsafe fn get_rw_txn(&self) -> Result<RwTransaction, db::Error> {
        // The returned transaction is only valid for *this* environment.
        Ok(self.env.begin_rw_txn()?)
    }

    pub unsafe fn new(env: Arc<Environment>, db: RawDB) -> Self {
        let db = DB::new(db);
        Self { env, db }
    }

    pub unsafe fn open(env: Arc<Environment>) -> Result<Self, db::Error> {
        let db = RawDB::open(&env, Some("user"))?;
        Ok(Self::new(env, db))
    }

    pub unsafe fn create(env: Arc<Environment>) -> Result<Self, db::Error> {
        let flags = DatabaseFlags::empty();
        let db = RawDB::create(&env, Some("user"), flags)?;
        Ok(Self::new(env, db))
    }

    pub fn get(&self, uid: &str) -> Result<Option<ArchivedValue<User>>, db::Error> {
        let txn = self.env.begin_ro_txn()?;
        self.db.get(&txn, &uid.as_bytes())
    }

    pub fn put(&self, uid: &str, user: &User) -> Result<(), db::Error> {
        let mut serializer = AllocSerializer::<1024>::default();
        serializer.serialize_value(user).expect("rkyv error");
        let v = serializer.into_serializer().into_inner();
        let value = ArchivedValue::new(v);

        let mut txn = self.env.begin_rw_txn()?;
        let flags = WriteFlags::empty();
        self.db.put(&mut txn, &uid.as_bytes(), &value, flags)?;
        txn.commit()?;
        Ok(())
    }

    pub fn put_txn(
        &self,
        txn: &mut RwTransaction,
        uid: &str,
        user: &User,
    ) -> Result<(), db::Error> {
        let mut serializer = AllocSerializer::<1024>::default();
        serializer.serialize_value(user).expect("rkyv error");
        let v = serializer.into_serializer().into_inner();
        let value = ArchivedValue::new(v);

        let flags = WriteFlags::empty();
        self.db.put(txn, &uid.as_bytes(), &value, flags)?;
        Ok(())
    }

    pub fn delete(&self, uid: &str) -> Result<(), db::Error> {
        let mut txn = self.env.begin_rw_txn()?;
        self.db.del(&mut txn, &uid)?;
        txn.commit()?;
        Ok(())
    }

    pub fn clear_txn(&self, txn: &mut RwTransaction) -> Result<(), db::Error> {
        self.db.clear(txn);
        Ok(())
    }

    pub fn get_all(&self) -> Result<Vec<(String, User)>, db::Error> {
        let txn = self.env.begin_ro_txn()?;
        let iter = self.db.get_all(&txn)?;
        let mut out = Vec::new();
        for (uid, user) in iter {
            let uid = unsafe { std::str::from_utf8_unchecked(uid).to_string() };
            let user: User =
                Deserialize::<User, _>::deserialize(user.as_ref(), &mut Infallible).unwrap();
            out.push((uid, user));
        }

        Ok(out)
    }
}
