use std::{
    mem::size_of,
    ops::Deref,
    ptr::NonNull,
    rc::Rc,
    sync::Arc,
    marker::PhantomData,
    hash::{
        Hash,
        Hasher,
        BuildHasher,
    },
    collections::hash_map::RandomState,
};

use rkyv::{
    Archive,
    Archived,
    archived_root,

    Serialize,
    Deserialize,

    ser::serializers::AllocScratchError,
};

use lmdb::{
    Database,
    Cursor,
    RoCursor,
    Iter,
};

pub use rkyv::{
    Fallible,
};
pub use lmdb::{
    Environment,

    DatabaseFlags,
    WriteFlags,

    Transaction,
    RoTransaction,
    RwTransaction,
};


#[derive(Debug, Clone)]
pub struct RawDB {
    db: Database,
}

impl RawDB {
    pub fn open(env: &Environment, name: Option<&str>) -> lmdb::Result<Self> {
        env.open_db(name).map(|db| Self { db })
    }
    
    pub fn create(env: &Environment, name: Option<&str>, flags: DatabaseFlags) -> lmdb::Result<Self> {
        env.create_db(name, flags).map(|db| Self { db })
    }

    pub fn get<'txn, T: Transaction, K>(&self, txn: &'txn T, key: &K) -> lmdb::Result<Option<&'txn [u8]>>
        where K: AsRef<[u8]>
    {
        match txn.get(self.db, key) {
            Ok(buf) => Ok(Some(buf)),
            Err(lmdb::Error::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
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
}

/// An read-only entry reference
pub struct EntryPtr<'txn, K, V> {
    key: &'txn K,
    val: &'txn V,
}

#[derive(Archive, Serialize, Deserialize)]
/// The entry as it is stored inside the database.
struct Entry<K: Archive, V: Archive> {
    key: K,
    val: V,
}

pub struct HashDB<'txn, K, V, S = RandomState> {
    db: RawDB,
    hash_builder: S,
    phantom: &'txn PhantomData<(K,V)>,
}

impl<K, V> HashDB<'_, K, V>
{
    pub fn create(env: &Environment, name: Option<&str>) -> lmdb::Result<Self> {
        Self::create_with_hasher(env, name, RandomState::new())
    }
    pub fn open(env: &Environment, name: Option<&str>) -> lmdb::Result<Self> {
        Self::open_with_hasher(env, name, RandomState::new())
    }
}

impl<K, V, S> HashDB<'_, K, V, S>
{
    pub fn create_with_hasher(env: &Environment, name: Option<&str>, hash_builder: S) -> lmdb::Result<Self> {
        let flags = DatabaseFlags::INTEGER_KEY | DatabaseFlags::DUP_SORT;
        let db = RawDB::create(env, name, flags)?;

        Ok(Self {
            db,
            hash_builder,
            phantom: &PhantomData,
        })
    }
    pub fn open_with_hasher(env: &Environment, name: Option<&str>, hash_builder: S) -> lmdb::Result<Self> {
        let db = RawDB::open(env, name)?;

        Ok(Self {
            db,
            hash_builder,
            phantom: &PhantomData,
        })
    }

}

impl<'txn, K, V, S> HashDB<'txn, K, V, S>
    where K: Eq + Hash + Archive,
          V: Archive,
          S: BuildHasher,
          K::Archived: PartialEq<K>,
{
    /// Retrieve an entry from the hashdb
    ///
    /// The result is a view pinned to the lifetime of the transaction. You can get owned Values
    /// using [`Deserialize`].
    pub fn get<T: Transaction>(&self, txn: &'txn T, key: &K) -> lmdb::Result<Option<&'txn Archived<Entry<K, V>>>>
    {
        let mut hasher = self.hash_builder.build_hasher();
        key.hash(&mut hasher);
        let hash = hasher.finish();

        let mut cursor = self.db.open_ro_cursor(txn)?;
        for res in cursor.iter_dup_of(&hash.to_ne_bytes()) {
            let (_keybuf, valbuf) = res?;
            let entry: &Archived<Entry<K, V>> = unsafe { archived_root::<Entry<K,V>>(valbuf.as_ref()) };

            if &entry.key == key {
                return Ok(Some(entry)) /*(EntryPtr {
                    key: &entry.key,
                    val: &entry.val,
                }))*/;
            }
        }

        Ok(None)
    }

    pub fn insert(&self, txn: &mut RwTransaction, entry: Archived<Entry<K, V>>) -> lmdb::Result<()> {

    }
}

/// Memory Fixpoint for a value in the DB
///
/// LMDB binds lifetimes of buffers to the transaction that returned the buffer. As long as this
/// transaction is not `commit()`ed, `abort()`ed or `reset()`ed the pages containing these values
/// are not returned into circulation.
/// This struct encodes this by binding a live reference to the Transaction to the returned
/// and interpreted buffer. The placeholder `T` is the container for the transaction. This may be a
/// plain `RoTransaction<'env>`, a `Rc<RoTxn>` (meaning Fix is !Send) or an `Arc<RoTxn>`, depending
/// on your needs.
pub struct Fix<T, V: Archive> {
    ptr: NonNull<V::Archived>,
    txn: T,
}
pub type PinnedGet<'env, V> = Fix<RoTransaction<'env>, V>;
pub type LocalKeep<'env, V> = Fix<Rc<RoTransaction<'env>>, V>;
pub type GlobalKeep<'env, V> = Fix<Arc<RoTransaction<'env>>, V>;

impl<'env, T, V> Fix<T, V>
    where T: AsRef<RoTransaction<'env>>,
          V: Archive,
{
    pub fn get(txn: T, db: &DB<V>, key: u64) -> lmdb::Result<Option<Self>> {
        match db.get(txn.as_ref(), &key.to_ne_bytes()) {
            Ok(buf) => Ok(Some(
                Self { 
                    ptr: unsafe { archived_root::<V>(buf.as_ref()).into() },
                    txn, 
                }
            )),
            Err(lmdb::Error::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
impl<'env, T, V> Deref for Fix<T, V>
    where T: AsRef<RoTransaction<'env>>,
          V: Archive,
{
    type Target = V::Archived;

    fn deref(&self) -> &Self::Target {
        // As long as the transaction is kept alive (which it is, because it's in self) state is a
        // valid pointer so this is safe.
        unsafe { self.ptr.as_ref() }
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
