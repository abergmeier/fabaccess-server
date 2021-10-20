use std::sync::Arc;
use super::Environment;
use super::AllocAdapter;
use super::DB;
use super::raw::RawDB;
use super::{DatabaseFlags, WriteFlags};
use crate::db::Result;
use super::Transaction;

use argon2;

type Adapter = AllocAdapter<String>;
#[derive(Clone)]
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
        let db = RawDB::open(&env, Some("pass"))?;
        Ok(Self::new(env, db))
    }

    pub unsafe fn create(env: Arc<Environment>) -> Result<Self> {
        let flags = DatabaseFlags::empty();
        let db = RawDB::create(&env, Some("pass"), flags)?;
        Ok(Self::new(env, db))
    }

    pub fn check_pw<P: AsRef<[u8]>>(&self, uid: &str, inpass: P) -> Result<Option<bool>> {
        let txn = self.env.begin_ro_txn()?;
        if let Some(pass) = self.db.get(&txn, &uid.as_bytes())? {
            Ok(argon2::verify_encoded(pass.as_str(), inpass.as_ref())
                .ok())
        } else {
            Ok(None)
        }
    }

    pub fn set_password<P: AsRef<[u8]>>(&self, uid: &str, password: P) -> Result<()> {
        let cfg = argon2::Config::default();
        let salt: [u8; 10] = rand::random();
        let enc = argon2::hash_encoded(password.as_ref(), &salt, &cfg)
            .expect("Hashing password failed for static valid config");

        let flags = WriteFlags::empty();
        let mut txn = self.env.begin_rw_txn()?;
        self.db.put(&mut txn, &uid.as_bytes(), &enc, flags)?;
        txn.commit()?;
        Ok(())
    }

    pub fn get_all(&self) -> Result<Vec<(String, String)>> {
        let txn = self.env.begin_ro_txn()?;
        let mut cursor = self.db.open_ro_cursor(&txn)?;
        let iter = cursor.iter_start();
        let mut out = Vec::new();
        for pass in iter {
            let (uid, pass) = pass?;
            let uid = unsafe { std::str::from_utf8_unchecked(uid).to_string() };
            let pass = unsafe { pass.as_str().to_string() };
            out.push((uid, pass));
        }

        Ok(out)
    }
}