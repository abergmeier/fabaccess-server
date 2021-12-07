use std::sync::Arc;
use std::path::PathBuf;
use std::str::FromStr;
use std::ops::{Deref, DerefMut};

use slog::Logger;

use crate::error::Result;
use crate::config::Config;

/// (Hashed) password database
pub mod pass;

/// User storage
pub mod user;

/// Access control storage
///
/// Stores&Retrieves Permissions and Roles
pub mod access;

/// Machine storage
///
/// Stores&Retrieves Machines
pub mod machine;

pub type MachineDB = machine::internal::Internal;
pub type UserDB = user::Internal;

#[derive(Clone)]
pub struct Databases {
    pub access: Arc<access::AccessControl>,
    pub machine: Arc<MachineDB>,
    pub userdb: Arc<UserDB>,
}

const LMDB_MAX_DB: u32 = 16;

impl Databases {
    pub fn new(log: &Logger, config: &Config) -> Result<Self> {

        // Initialize the LMDB environment. This blocks until the mmap() finishes
        info!(log, "LMDB env");
        let env = lmdb::Environment::new()
            .set_flags(lmdb::EnvironmentFlags::MAP_ASYNC | lmdb::EnvironmentFlags::NO_SUB_DIR)
            .set_max_dbs(LMDB_MAX_DB as libc::c_uint)
            .open(config.db_path.as_path())?;

        // Start loading the machine database, authentication system and permission system
        // All of those get a custom logger so the source of a log message can be better traced and
        // filtered
        let env = Arc::new(env);
        let mdb = machine::init(log.new(o!("system" => "machines")), &config, env.clone())?;

        let permdb = access::init(log.new(o!("system" => "permissions")), &config, env.clone())?;
        let ac = access::AccessControl::new(permdb);

        let userdb = user::init(log.new(o!("system" => "users")), &config, env.clone())?;

        Ok(Self {
            access: Arc::new(ac),
            machine: Arc::new(mdb),
            userdb: Arc::new(userdb),
        })
    }
}

use lmdb::{
    Environment,
    Database,
    Transaction,
    RoTransaction,
    RwTransaction,
    WriteFlags,
    Cursor,
    RoCursor,
    RwCursor,
    Iter,
};

#[derive(Debug, Clone)]
pub struct DB {
    env: Arc<Environment>,
    db: Database,
}

impl DB {
    pub fn new(env: Arc<Environment>, db: Database) -> Self {
        Self { env, db }
    }

    pub fn open(env: Arc<Environment>, name: &str) -> lmdb::Result<Self> {
        env.open_db(Some(name)).map(|db| { Self::new(env.clone(), db) })
    }

    pub fn get<'txn, T: Transaction, K>(&self, txn: &'txn T, key: &K) -> lmdb::Result<&'txn [u8]> 
        where K: AsRef<[u8]>
    {
        txn.get(self.db, key)
    }

    pub fn put<K, V>(&self, txn: &mut RwTransaction, key: &K, value: &V, flags: WriteFlags)
        -> lmdb::Result<()>
        where K: AsRef<[u8]>,
              V: AsRef<[u8]>,
    {
        txn.put(self.db, key, value, flags)
    }

    pub fn reserve<'txn, K>(&self, txn: &'txn mut RwTransaction, key: &K, size: usize, flags: WriteFlags) 
        -> lmdb::Result<&'txn mut [u8]>
        where K: AsRef<[u8]>
    {
        txn.reserve(self.db, key, size, flags)
    }

    pub fn del<K, V>(&self, txn: &mut RwTransaction, key: &K, value: Option<&V>) -> lmdb::Result<()>
        where K: AsRef<[u8]>,
              V: AsRef<[u8]>,
    {
        txn.del(self.db, key, value.map(AsRef::as_ref))
    }

    pub fn iter<'txn, C: Cursor<'txn>>(&self, cursor: &'txn mut C) -> Iter<'txn> {
        cursor.iter_start()
    }

    pub fn open_ro_cursor<'txn, T: Transaction>(&self, txn: &'txn T) -> lmdb::Result<RoCursor<'txn>> {
        txn.open_ro_cursor(self.db)
    }

    pub fn begin_ro_txn<'env>(&'env self) -> lmdb::Result<RoTransaction<'env>> {
        self.env.begin_ro_txn()
    }

    pub fn begin_rw_txn<'env>(&'env self) -> lmdb::Result<RwTransaction<'env>> {
        self.env.begin_rw_txn()
    }
}

use std::result::Result as StdResult;
use serde::{Serialize, Deserialize};
use bincode::Options;

pub trait DatabaseAdapter {
    type Key: ?Sized;
    type Err: From<lmdb::Error> + From<bincode::Error>;

    fn serialize_key(key: &Self::Key) -> &[u8];
    fn deserialize_key<'de>(input: &'de [u8]) -> StdResult<&'de Self::Key, Self::Err>;
}

// Should we for some reason ever need to have different Options for different adapters we can have
// this in the DatabaseAdapter trait too
fn bincode_default() -> impl bincode::Options {
    bincode::DefaultOptions::new()
        .with_varint_encoding()
}

use std::marker::PhantomData;

pub struct Objectstore<'a, A, V: ?Sized> {
    pub db: DB,
    adapter: PhantomData<A>,
    marker: PhantomData<&'a V>
}

impl<A, V: ?Sized> Objectstore<'_, A, V> {
    pub fn new(db: DB) -> Self {
        Self {
            db: db,
            adapter: PhantomData,
            marker: PhantomData,
        }
    }
}

impl<'txn, A, V> Objectstore<'txn, A, V>
    where A: DatabaseAdapter,
          V: ?Sized + Serialize + Deserialize<'txn>,
{
    pub fn get<T: Transaction>(&self, txn: &'txn T, key: &A::Key) 
        -> StdResult<Option<V>, A::Err>
    {
        let opts = bincode_default();

        self.db.get(txn, &A::serialize_key(key))
            .map_or_else(
                |err| match err {
                    lmdb::Error::NotFound => Ok(None),
                    e => Err(e.into()),
                },
                |ok| opts.deserialize(ok)
                    .map_err(|e| e.into())
                    .map(Option::Some)
            )
    }

    /// Update `value` in-place from the database
    /// 
    /// Returns `Ok(false)` if the key wasn't found. If this functions returns an error `value`
    /// will be in an indeterminate state where some parts may be updated from the db.
    pub fn get_in_place<T: Transaction>(&self, txn: &'txn T, key: &A::Key, value: &mut V)
        -> StdResult<bool, A::Err>
    {
        let opts = bincode_default();

        self.db.get(txn, &A::serialize_key(key))
            .map_or_else(
                |err| match err {
                    lmdb::Error::NotFound => Ok(false),
                    e => Err(e.into()),
                },
                |ok| opts.deserialize_in_place_buffer(ok, value)
                    .map_err(|e| e.into())
                    .map(|()| true)
            )
    }

    pub fn iter<T: Transaction>(&self, txn: &'txn T) -> StdResult<ObjectIter<'txn, A, V>, A::Err> {
        let mut cursor = self.db.open_ro_cursor(txn)?;
        let iter = cursor.iter_start();
        Ok(ObjectIter::new(cursor, iter))
    }

    pub fn put(&self, txn: &'txn mut RwTransaction, key: &A::Key, value: &V, flags: lmdb::WriteFlags)
        -> StdResult<(), A::Err>
    {
        let opts = bincode::DefaultOptions::new()
            .with_varint_encoding();

        // Serialized values are always at most as big as their memory representation.
        // So even if usize is 32 bit this is safe given no segmenting is taking place.
        let bufsize = opts.serialized_size(value)? as usize;

        let buffer = self.db.reserve(txn, &A::serialize_key(key), bufsize, flags)?;

        opts.serialize_into(buffer, value).map_err(|e| e.into())
    }

    pub fn del(&self, txn: &'txn mut RwTransaction, key: &A::Key)
        -> StdResult<(), A::Err>
    {
        self.db.del::<&[u8], &[u8]>(txn, &A::serialize_key(key), None).map_err(|e| e.into())
    }
}

pub struct ObjectIter<'txn, A, V: ?Sized> {
    cursor: RoCursor<'txn>,
    inner: Iter<'txn>,

    adapter: PhantomData<A>,
    marker: PhantomData<&'txn V>,
}

impl<'txn, A, V: ?Sized> ObjectIter<'txn, A, V> {
    pub fn new(cursor: RoCursor<'txn>, inner: Iter<'txn>) -> Self {
        let marker = PhantomData;
        let adapter = PhantomData;
        Self { cursor, inner, adapter, marker }
    }
}

impl<'txn, A, V> Iterator for ObjectIter<'txn, A, V>
    where A: DatabaseAdapter,
          V: ?Sized + Serialize + Deserialize<'txn>,
{
    type Item = StdResult<V, A::Err>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()?
            .map_or_else(
                |err| Some(Err(err.into())),
                |(_, v)| Some(bincode_default().deserialize(v).map_err(|e| e.into()))
            )
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::result::Result;
    use std::ops::Deref;

    use lmdb::{
        EnvironmentFlags as EF,
        DatabaseFlags as DF,
        WriteFlags as WF,
    };

    pub struct TempEnv {
        dir: tempfile::TempDir,
        env: Arc<Environment>,
    }

    impl Deref for TempEnv {
        type Target = Arc<Environment>;
        fn deref(&self) -> &Self::Target {
            &self.env
        }
    }

    pub fn open_test_env() -> TempEnv {
        let dir = tempfile::tempdir().expect("Failed to create tempdir for testdb");
        let env = Environment::new()
            .set_flags(EF::NO_SYNC | EF::WRITE_MAP)
            .open(dir.path()).expect("Failed to open lmdb");
        let env = Arc::new(env);

        TempEnv { dir, env }
    }

    struct TestAdapter;

    #[derive(Debug)]
    enum TestErr {
        Utf8(std::str::Utf8Error),
        Binc(Box<bincode::ErrorKind>),
        LMDB(lmdb::Error),
    }

    impl From<lmdb::Error> for TestErr {
        fn from(e: lmdb::Error) -> TestErr {
            TestErr::LMDB(e)
        }
    }

    impl From<std::str::Utf8Error> for TestErr {
        fn from(e: std::str::Utf8Error) -> TestErr {
            TestErr::Utf8(e)
        }
    }

    impl From<bincode::Error> for TestErr {
        fn from(e: bincode::Error) -> TestErr {
            TestErr::Binc(e)
        }
    }

    impl DatabaseAdapter for TestAdapter {
        type Key = str;
        type Err = TestErr;

        fn serialize_key(key: &Self::Key) -> &[u8] {
            key.as_bytes()
        }
        fn deserialize_key<'de>(input: &'de [u8]) -> Result<&'de Self::Key, Self::Err> {
            std::str::from_utf8(input).map_err(|e| e.into())
        }
    }

    type TestDB<'txn> = Objectstore<'txn, TestAdapter, &'txn str>;

    #[test]
    fn simple_get() {
        let e = open_test_env();
        let ldb = e.create_db(None, DF::empty()).expect("Failed to create lmdb db");

        let db = DB::new(e.env.clone(), ldb);

        let adapter = TestAdapter;
        let testdb = TestDB::new(db.clone());

        let mut val = "value";
        let mut txn = db.begin_rw_txn().expect("Failed to being rw txn");
        testdb.put(&mut txn, "key", &val, WF::empty()).expect("Failed to insert");
        testdb.put(&mut txn, "key2", &val, WF::empty()).expect("Failed to insert");
        testdb.put(&mut txn, "key3", &val, WF::empty()).expect("Failed to insert");
        testdb.put(&mut txn, "key4", &val, WF::empty()).expect("Failed to insert");
        testdb.put(&mut txn, "key5", &val, WF::empty()).expect("Failed to insert");
        txn.commit().expect("commit failed");

        {
            let txn;
            txn = db.begin_ro_txn().unwrap();

            let val = testdb.get(&txn, "key").expect("Failed to retrieve");
            assert_eq!(Some("value"), val);
        }

        {
            let val2 = "longer_value";
            let mut txn = db.begin_rw_txn().unwrap();
            testdb.put(&mut txn, "key", &val2, WF::empty()).expect("Failed to update");
            txn.commit().unwrap();
        }

        {
            let txn = db.begin_ro_txn().unwrap();
            let found = testdb.get_in_place(&txn, "key", &mut val).expect("Failed to retrieve update");
            assert!(found);
            assert_eq!("longer_value", val);
        }

        {
            let txn = db.begin_ro_txn().unwrap();
            let mut it = testdb.iter(&txn).unwrap();
            assert_eq!("longer_value", it.next().unwrap().unwrap());
            let mut i = 0;
            while let Some(e) = it.next() {
                assert_eq!("value", e.unwrap());
                i += 1;
            }
            assert_eq!(i, 4)
        }
    }
}
