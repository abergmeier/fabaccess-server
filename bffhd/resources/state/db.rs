use thiserror::Error;

use crate::db;
use crate::db::{AlignedAdapter, ArchivedValue, RawDB, DB};
use lmdb::{DatabaseFlags, Environment, EnvironmentFlags, Transaction, WriteFlags};
use miette::Diagnostic;
use std::fmt::Debug;
use std::{path::Path, sync::Arc};

use crate::resources::state::State;

#[derive(Debug, Clone)]
pub struct StateDB {
    env: Arc<Environment>,
    db: DB<AlignedAdapter<State>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Error, Diagnostic)]
pub enum StateDBError {
    #[error("opening the state db environment failed")]
    #[diagnostic(
        code(bffh::db::state::open_env),
        help("does the parent directory for state_db exist?")
    )]
    OpenEnv(#[source] db::Error),
    #[error("opening the state db failed")]
    #[diagnostic(code(bffh::db::state::open))]
    Open(#[source] db::Error),
    #[error("creating the state db failed")]
    #[diagnostic(code(bffh::db::state::create))]
    Create(#[source] db::Error),
}

impl StateDB {
    pub fn open_env<P: AsRef<Path>>(path: P) -> Result<Arc<Environment>, StateDBError> {
        Environment::new()
            .set_flags(
                EnvironmentFlags::WRITE_MAP
                    | EnvironmentFlags::NO_SUB_DIR
                    | EnvironmentFlags::NO_TLS
                    | EnvironmentFlags::NO_READAHEAD,
            )
            .set_max_dbs(8)
            .open(path.as_ref())
            .map(Arc::new)
            .map_err(|e| StateDBError::OpenEnv(e.into()))
    }

    fn new(env: Arc<Environment>, db: RawDB) -> Self {
        let db = DB::new(db);
        Self { env, db }
    }

    pub fn open_with_env(env: Arc<Environment>) -> Result<Self, StateDBError> {
        let db = RawDB::open(&env, Some("state"))
            .map_err(|e| StateDBError::Open(e.into()))?;
        Ok(Self::new(env, db))
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StateDBError> {
        let env = Self::open_env(path)?;
        Self::open_with_env(env)
    }

    pub fn create_with_env(env: Arc<Environment>) -> Result<Self, StateDBError> {
        let flags = DatabaseFlags::empty();
        let db = RawDB::create(&env, Some("state"), flags)
            .map_err(|e| StateDBError::Create(e.into()))?;

        Ok(Self::new(env, db))
    }

    pub fn create<P: AsRef<Path>>(path: P) -> Result<Self, StateDBError> {
        let env = Self::open_env(path)?;
        Self::create_with_env(env)
    }

    pub fn begin_ro_txn(&self) -> Result<impl Transaction + '_, db::Error> {
        self.env.begin_ro_txn().map_err(db::Error::from)
    }

    pub fn get(&self, key: impl AsRef<[u8]>) -> Result<Option<ArchivedValue<State>>, db::Error> {
        let txn = self.env.begin_ro_txn()?;
        self.db.get(&txn, &key.as_ref())
    }

    pub fn get_all<'txn, T: Transaction>(
        &self,
        txn: &'txn T,
    ) -> Result<impl IntoIterator<Item = (&'txn [u8], ArchivedValue<State>)>, db::Error> {
        self.db.get_all(txn)
    }

    pub fn put(&self, key: &impl AsRef<[u8]>, val: &ArchivedValue<State>) -> Result<(), db::Error> {
        let mut txn = self.env.begin_rw_txn()?;
        let flags = WriteFlags::empty();
        self.db.put(&mut txn, key, val, flags)?;
        Ok(txn.commit()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::ops::Deref;
}
