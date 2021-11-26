use std::sync::Arc;
use crate::db::{RawDB, DB, AllocAdapter, Environment, Result};
use crate::db::{DatabaseFlags, LMDBorrow, RoTransaction, WriteFlags, };
use super::User;

use rkyv::{Deserialize, Archived};

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