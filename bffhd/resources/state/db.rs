use std::{
    sync::Arc,
    path::Path,
};

use rkyv::Archived;

use crate::db::{
    DB,
    Environment,

    EnvironmentFlags,
    DatabaseFlags,
    WriteFlags,

    Adapter,
    AllocAdapter,
    DBError,

    Transaction,
    RoTransaction,
    RwTransaction,

    LMDBorrow,
};

use crate::resources::state::State;

type StateAdapter = AllocAdapter<State>;

/// State Database containing the currently set state
#[derive(Debug)]
pub struct StateDB {
    /// The environment for all the databases below
    env: Arc<Environment>,

    input: DB<StateAdapter>,
    output: DB<StateAdapter>,

    // TODO: Index resources name/id/uuid -> u64
}

impl StateDB {
    fn open_env<P: AsRef<Path>>(path: P) -> lmdb::Result<Environment> {
        Environment::new()
            .set_flags( EnvironmentFlags::WRITE_MAP 
                      | EnvironmentFlags::NO_SUB_DIR 
                      | EnvironmentFlags::NO_TLS
                      | EnvironmentFlags::NO_READAHEAD)
            .set_max_dbs(2)
            .open(path.as_ref())
    }

    fn new(env: Environment, input: DB<StateAdapter>, output: DB<StateAdapter>) -> Self {
        Self { env: Arc::new(env), input, output }
    }

    pub fn open<P: AsRef<Path>>(path: P) -> lmdb::Result<Self> {
        let env = Self::open_env(path)?;
        let input = unsafe { DB::open(&env, Some("input"))?  };
        let output = unsafe { DB::open(&env, Some("output"))?  };

        Ok(Self::new(env, input, output))
    }

    pub fn create<P: AsRef<Path>>(path: P) -> lmdb::Result<Self> {
        let flags = DatabaseFlags::empty();
        let env = Self::open_env(path)?;
        let input = unsafe { DB::create(&env, Some("input"), flags)?  };
        let output = unsafe { DB::create(&env, Some("output"), flags)?  };

        Ok(Self::new(env, input, output))
    }

    fn update_txn(&self, txn: &mut RwTransaction, key: impl AsRef<[u8]>, input: &State, output: &State)
        -> Result<(), DBError>
    {
        let flags = WriteFlags::empty();
        let k = key.as_ref();
        self.input.put(txn, &k, input, flags)?;
        self.output.put(txn, &k, output, flags)?;
        Ok(())
    }

    pub fn update(&self, key: impl AsRef<[u8]>, input: &State, output: &State)
        -> Result<(), DBError>
    {
        let mut txn = self.env.begin_rw_txn().map_err(StateAdapter::from_db_err)?;
        self.update_txn(&mut txn, key, input, output)?;

        txn.commit().map_err(StateAdapter::from_db_err)
    }

    fn get(&self, db: &DB<StateAdapter>, key: impl AsRef<[u8]>)
        -> Result<Option<LMDBorrow<RoTransaction, Archived<State>>>, DBError> 
    {
        let txn = self.env.begin_ro_txn().map_err(StateAdapter::from_db_err)?;
        if let Some(state) = db.get(&txn, &key.as_ref())? {
            let ptr = state.into();
            Ok(Some(unsafe { LMDBorrow::new(ptr, txn) }))
        } else {
            Ok(None)
        }
    }

    #[inline(always)]
    pub fn get_input(&self, key: impl AsRef<[u8]>)
        -> Result<Option<LMDBorrow<RoTransaction, Archived<State>>>, DBError> 
    { self.get(&self.input, key) }

    #[inline(always)]
    pub fn get_output(&self, key: impl AsRef<[u8]>)
        -> Result<Option<LMDBorrow<RoTransaction, Archived<State>>>, DBError> 
    { self.get(&self.output, key) }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::resource::state::value::Vec3u8;
    use crate::resource::state::value::{OID_COLOUR, OID_POWERED, OID_INTENSITY};
    use std::ops::Deref;

    #[test]
    fn construct_state() {
        let tmpdir = tempfile::tempdir().unwrap();
        let mut tmppath = tmpdir.path().to_owned();
        tmppath.push("db");
        let db = StateDB::create(tmppath).unwrap();
        let b = State::build()
            .add(OID_COLOUR.clone(), Box::new(Vec3u8 { a: 1, b: 2, c: 3}))
            .add(OID_POWERED.clone(), Box::new(true))
            .add(OID_INTENSITY.clone(), Box::new(1023))
            .finish();
        println!("({}) {:?}", b.hash(), b);

        let c = State::build()
            .add(OID_COLOUR.clone(), Box::new(Vec3u8 { a: 1, b: 2, c: 3}))
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
