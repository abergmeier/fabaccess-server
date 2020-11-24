use std::sync::Arc;

use argon2;
use lmdb::{Environment, Transaction, RwTransaction, Cursor};
use rand::prelude::*;
use slog::Logger;

use crate::error::Result;

pub struct PassDB {
    log: Logger,
    env: Arc<Environment>,
    db: lmdb::Database,
}

impl PassDB {
    pub fn new(log: Logger, env: Arc<Environment>, db: lmdb::Database) -> Self {
        Self { log, env, db }
    }

    pub fn check<T: Transaction>(&self, txn: &T, authcid: &str, password: &[u8]) -> Result<Option<bool>> {
        match txn.get(self.db, &authcid.as_bytes()) {
            Ok(bytes) => {
                let encoded = unsafe { std::str::from_utf8_unchecked(bytes) };
                let res = argon2::verify_encoded(encoded, password)?;
                Ok(Some(res))
            },
            Err(lmdb::Error::NotFound) => { Ok(None) },
            Err(e) => { Err(e.into()) },
        }
    }

    pub fn store(&self, txn: &mut RwTransaction, authcid: &str, password: &[u8]) -> Result<()> {
        let config = argon2::Config::default();
        let salt: [u8; 16] = rand::random();
        let hash = argon2::hash_encoded(password, &salt, &config)?;
        txn.put(self.db, &authcid.as_bytes(), &hash.as_bytes(), lmdb::WriteFlags::empty())
            .map_err(Into::into)
    }
}
