use std::sync::Arc;

use slog::Logger;
use lmdb::{Environment, Transaction, RwTransaction, Cursor};

use crate::error::Result;

use super::*;

#[derive(Clone, Debug)]
pub struct Internal {
    log: Logger,
    env: Arc<Environment>,
    db: lmdb::Database,
}

impl Internal {
    pub fn new(log: Logger, env: Arc<Environment>, db: lmdb::Database) -> Self {
        Self { log, env, db }
    }

    pub fn get_user_txn<T: Transaction>(&self, txn: &T, uid: &str) -> Result<Option<User>> {
        match txn.get(self.db, &uid.as_bytes()) {
            Ok(bytes) => {
                Ok(Some(flexbuffers::from_slice(bytes)?))
            },
            Err(lmdb::Error::NotFound) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    pub fn get_user(&self, uid: &str) -> Result<Option<User>> {
        let txn = self.env.begin_ro_txn()?;
        self.get_user_txn(&txn, uid)
    }

    pub fn put_user_txn(&self, txn: &mut RwTransaction, uid: &str, user: &User) -> Result<()> {
        let bytes = flexbuffers::to_vec(user)?;
        txn.put(self.db, &uid.as_bytes(), &bytes, lmdb::WriteFlags::empty())?;

        Ok(())
    }
    pub fn put_user(&self, uid: &str, user: &User) -> Result<()> {
        let mut txn = self.env.begin_rw_txn()?;
        self.put_user_txn(&mut txn, uid, user)?;
        txn.commit()?;

        Ok(())
    }

    pub fn list_users(&self) -> Result<Vec<User>> {
        let txn = self.env.begin_ro_txn()?;
        Ok(self.list_users_txn(&txn)?.collect())
    }
    pub fn list_users_txn<T: Transaction>(&self, txn: &T) -> Result<impl Iterator<Item=User>> {
       let mut cursor = txn.open_ro_cursor(self.db)?;
       Ok(cursor.iter_start().map(|buf| {
           let (_kbuf, vbuf) = buf.unwrap();
           flexbuffers::from_slice(vbuf).unwrap()
       }))
    }

    pub fn login(&self, uid: &str, password: &[u8]) -> Result<Option<User>> {
        let txn = self.env.begin_ro_txn()?;
        Ok(self.get_user_txn(&txn, uid)?
            .filter(|user| {
                user.data.passwd.is_some()
                && argon2::verify_encoded(user.data.passwd.as_ref().unwrap(), password).unwrap_or(false)
            }))
    }
}
