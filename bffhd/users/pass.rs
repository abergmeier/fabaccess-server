use std::sync::Arc;
use rand::RngCore;
use crate::db::{RawDB, DB, AllocAdapter, Environment, Result, DBError};
use crate::db::{DatabaseFlags, WriteFlags};

use rkyv::{Serialize, Deserialize, Archived, Archive};

#[repr(transparent)]
#[derive(Debug, Clone, Eq, PartialEq, Archive, Serialize, Deserialize)]
pub struct Password(String);

fn check_password(stored: &Archived<Password>, input: &[u8]) -> argon2::Result<bool> {
    argon2::verify_encoded(stored.0.as_str(), input)
}

type Adapter = AllocAdapter<Password>;

#[derive(Clone, Debug)]
/// Internal Password Database
pub struct PassDB {
    env: Arc<Environment>,
    db: DB<Adapter>,
}

impl PassDB {
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

    /// Verify if the given password matches for the given user.
    pub fn verify_password(&self, uid: &str, password: &[u8])
        -> Result<Option<argon2::Result<bool>>>
    {
        let txn = self.env.begin_ro_txn()?;
        if let Some(stored) = self.db.get(&txn, &uid.as_bytes())? {
            Ok(Some(check_password(stored, password)))
        } else {
            Ok(None)
        }
    }

    /// Set or update a password for a given uid.
    ///
    /// The given uid must not be "" and the given password must be 1..=1024 bytes.
    pub fn set_password(&self, uid: &str, password: &[u8]) -> Result<()> {
        debug_assert!(!uid.is_empty());
        debug_assert!(0 < password.len() && password.len() <= 1024);

        let config = argon2::Config::default();
        let mut salt: [u8; 16] = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut salt);
        let encoded = argon2::hash_encoded(password, &salt, &config)
            .expect("Hashing given user password failed");
        let pwd = Password(encoded);

        let mut txn = self.env.begin_rw_txn()?;
        self.db.put(&mut txn, &uid.as_bytes(), &pwd, WriteFlags::empty())?;
        Ok(())
    }

    /// Delete an password entry from the database.
    ///
    /// Returns `Ok(false)` if no entry existed for the given user.
    /// Thus, if this function returns `Ok(_)` you can be sure this db contains no password hash
    /// for the given uid.
    pub fn delete_password(&self, uid: &str) -> Result<bool> {
        let mut txn = self.env.begin_rw_txn()?;
        match self.db.del(&mut txn, &uid.as_bytes()) {
            Ok(_) => Ok(true),
            Err(DBError::LMDB(lmdb::Error::NotFound)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Return all entries in this db in the form [(uid, password hash)].
    pub fn get_all(&self) -> Result<Vec<(String, Password)>> {
        let txn = self.env.begin_ro_txn()?;
        let mut cursor = self.db.open_ro_cursor(&txn)?;
        let iter = cursor.iter_start();
        let mut out = Vec::new();
        let mut deserializer = rkyv::Infallible;
        for passentry in iter {
            let (uid, password) = passentry?;
            let uid = unsafe { std::str::from_utf8_unchecked(uid).to_string() };
            let password: Password = password.deserialize(&mut deserializer).unwrap();
            out.push((uid, password));
        }

        Ok(out)
    }
}