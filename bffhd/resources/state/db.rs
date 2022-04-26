use crate::db;
use crate::db::{ArchivedValue, RawDB, DB, AlignedAdapter};
use lmdb::{
    DatabaseFlags, Environment, EnvironmentFlags, Transaction,
    WriteFlags,
};
use std::{path::Path, sync::Arc};

use crate::resources::state::State;

#[derive(Debug, Clone)]
pub struct StateDB {
    env: Arc<Environment>,
    db: DB<AlignedAdapter<State>>,
}

impl StateDB {
    pub fn open_env<P: AsRef<Path>>(path: P) -> lmdb::Result<Arc<Environment>> {
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
    }

    fn new(env: Arc<Environment>, db: RawDB) -> Self {
        let db = DB::new(db);
        Self { env, db }
    }

    pub fn open_with_env(env: Arc<Environment>) -> lmdb::Result<Self> {
        let db = unsafe { RawDB::open(&env, Some("state"))? };
        Ok(Self::new(env, db))
    }

    pub fn open<P: AsRef<Path>>(path: P) -> lmdb::Result<Self> {
        let env = Self::open_env(path)?;
        Self::open_with_env(env)
    }

    pub fn create_with_env(env: Arc<Environment>) -> lmdb::Result<Self> {
        let flags = DatabaseFlags::empty();
        let db = unsafe { RawDB::create(&env, Some("state"), flags)? };

        Ok(Self::new(env, db))
    }

    pub fn create<P: AsRef<Path>>(path: P) -> lmdb::Result<Self> {
        let env = Self::open_env(path)?;
        Self::create_with_env(env)
    }

    pub fn begin_ro_txn(&self) -> Result<impl Transaction + '_, db::Error> {
        self.env.begin_ro_txn()
    }

    pub fn get(&self, key: impl AsRef<[u8]>) -> Result<Option<ArchivedValue<State>>, db::Error> {
        let txn = self.env.begin_ro_txn()?;
        self.db.get(&txn, &key.as_ref())
    }

    pub fn get_all<'txn, T: Transaction>(
        &self,
        txn: &'txn T,
    ) -> Result<impl IntoIterator<Item = (&'txn [u8], ArchivedValue<State>)>, db::Error, > {
        self.db.get_all(txn)
    }

    pub fn put(&self, key: &impl AsRef<[u8]>, val: &ArchivedValue<State>) -> Result<(), db::Error> {
        let mut txn = self.env.begin_rw_txn()?;
        let flags = WriteFlags::empty();
        self.db.put(&mut txn, key, val, flags)?;
        txn.commit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::resource::state::value::Vec3u8;
    use crate::resource::state::value::{OID_COLOUR, OID_INTENSITY, OID_POWERED};
    use std::ops::Deref;

    #[test]
    fn construct_state() {
        let tmpdir = tempfile::tempdir().unwrap();
        let mut tmppath = tmpdir.path().to_owned();
        tmppath.push("db");
        let db = StateDB::create(tmppath).unwrap();
        let b = State::build()
            .add(OID_COLOUR.clone(), Box::new(Vec3u8 { a: 1, b: 2, c: 3 }))
            .add(OID_POWERED.clone(), Box::new(true))
            .add(OID_INTENSITY.clone(), Box::new(1023))
            .finish();
        println!("({}) {:?}", b.hash(), b);

        let c = State::build()
            .add(OID_COLOUR.clone(), Box::new(Vec3u8 { a: 1, b: 2, c: 3 }))
            .add(OID_POWERED.clone(), Box::new(true))
            .add(OID_INTENSITY.clone(), Box::new(1023))
            .finish();

        let key = rand::random();
        db.update(key, &b, &c).unwrap();
        let d = db.get_input(key).unwrap().unwrap();
        let e = db.get_output(key).unwrap().unwrap();
        assert_eq!(&b, d.deref());
        assert_eq!(&c, e.deref());
    }
}
