use std::{
    sync::Arc,
    path::Path,
};

use rkyv::Archived;

use super::{
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

use crate::state::State;

type StateAdapter = AllocAdapter<State>;

/// State Database containing the currently set state
#[derive(Clone, Debug)]
pub struct StateDB {
    /// The environment for all the databases below
    env: Arc<Environment>,

    input: DB<StateAdapter>,
    output: DB<StateAdapter>,

    // TODO: Index resource name/id/uuid -> u64
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

    pub fn init<P: AsRef<Path>>(path: P) -> lmdb::Result<Self> {
        let env = Self::open_env(path)?;
        let input = unsafe {
            DB::create(&env, Some("input"), DatabaseFlags::INTEGER_KEY)?
        };
        let output = unsafe {
            DB::create(&env, Some("output"), DatabaseFlags::INTEGER_KEY)?
        };

        Ok(Self::new(env, input, output))
    }

    pub fn open<P: AsRef<Path>>(path: P) -> lmdb::Result<Self> {
        let env = Self::open_env(path)?;
        let input = unsafe { DB::open(&env, Some("input"))?  };
        let output = unsafe { DB::open(&env, Some("output"))?  };

        Ok(Self::new(env, input, output))
    }

    fn update_txn(&self, txn: &mut RwTransaction, key: u64, input: &State, output: &State)
        -> Result<(), DBError>
    {
        let flags = WriteFlags::empty();
        let k = key.to_ne_bytes();
        self.input.put(txn, &k, input, flags)?;
        self.output.put(txn, &k, output, flags)?;
        Ok(())
    }

    pub fn update(&self, key: u64, input: &State, output: &State) 
        -> Result<(), DBError>
    {
        let mut txn = self.env.begin_rw_txn().map_err(StateAdapter::from_db_err)?;
        self.update_txn(&mut txn, key, input, output)?;

        txn.commit().map_err(StateAdapter::from_db_err)
    }

    fn get(&self, db: &DB<StateAdapter>, key: u64)
        -> Result<Option<LMDBorrow<RoTransaction, Archived<State>>>, DBError> 
    {
        let txn = self.env.begin_ro_txn().map_err(StateAdapter::from_db_err)?;
        if let Some(state) = db.get(&txn, &key.to_ne_bytes())? {
            let ptr = state.into();
            Ok(Some(unsafe { LMDBorrow::new(ptr, txn) }))
        } else {
            Ok(None)
        }
    }

    #[inline(always)]
    pub fn get_input(&self, key: u64)
        -> Result<Option<LMDBorrow<RoTransaction, Archived<State>>>, DBError> 
    { self.get(&self.input, key) }

    #[inline(always)]
    pub fn get_output(&self, key: u64)
        -> Result<Option<LMDBorrow<RoTransaction, Archived<State>>>, DBError> 
    { self.get(&self.output, key) }

    pub fn accessor(&self, key: u64) -> StateAccessor {
        StateAccessor::new(key, self.clone())
    }
}

#[derive(Debug)]
pub struct StateAccessor {
    key: u64,
    db: StateDB
}

impl StateAccessor {
    pub fn new(key: u64, db: StateDB) -> Self {
        Self { key, db }
    }

    pub fn get_input(&self)
        -> Result<Option<LMDBorrow<RoTransaction, Archived<State>>>, DBError>
    {
        self.db.get_input(self.key)
    }

    pub fn get_output(&self)
        -> Result<Option<LMDBorrow<RoTransaction, Archived<State>>>, DBError>
    {
        self.db.get_output(self.key)
    }

    pub fn set(&self, input: &State, output: &State) -> Result<(), DBError> {
        self.db.update(self.key, input, output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::db::tests::open_test_env;
    use lmdb::{
        EnvironmentFlags as EF,
        DatabaseFlags as DF,
        WriteFlags as WF,
    };

    use rkyv::Infallible;
    use rkyv::ser::serializers::AllocSerializer;
    use rkyv::archived_root;
    use rkyv::util::archived_value;

    #[test]
    fn construct_state() {
        let b = State::build()
            .add("Colour".to_string(), Vec3u8 { a: 1, b: 2, c: 3})
            .add("Powered".to_string(), Bool(true))
            .add("Intensity".to_string(), UInt32(4242))
            .finish();

        println!("({}) {:?}", b.hash(), b);

        let mut serializer = AllocSerializer::<256>::default();
        let pos = serializer.serialize_value(&b).unwrap();
        let buf = serializer.into_serializer().into_inner();

        println!("Encsize: {}", buf.len());

        let archived_state = unsafe {
            archived_value::<State>(buf.as_ref(), pos)
        };
        let s: State = archived_state.deserialize(&mut Infallible).unwrap();

        println!("({}) {:?}", pos, s);
    }

    #[test]
    fn function_name_test() {
        let te = open_text_env();
        let ildb = e.create_db(Some("input"), DF::empty()).expect("Failed to create db file");
        let oldb = e.create_db(Some("output"), DF::empty()).expect("Failed to create db file");

        let idb = DB::new(e.env.clone(), ildb);
        let odb = DB::new(e.env.clone(), oldb);
        let db = StateDB::new(idb, odb);
    }
}
