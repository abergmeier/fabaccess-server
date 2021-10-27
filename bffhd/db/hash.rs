use std::{
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
    Serialize,
    Deserialize,
    Fallible,
};

use super::{
    DB,
    Adapter,
    OutputBuffer,

    Environment,

    DatabaseFlags,
    WriteFlags,

    Transaction,
    RwTransaction,
};


#[derive(Archive, Serialize, Deserialize)]
/// The entry as it is stored inside the database.
pub struct Entry<K: Archive, V: Archive> {
    pub key: K,
    pub val: V,
}

#[derive(Clone, Copy)]
pub struct HashAdapter<K, A> {
    k: PhantomData<K>,
    a: PhantomData<A>,
}
impl<K, A> HashAdapter<K, A> {
    pub fn new() -> Self {
        Self { k: PhantomData, a: PhantomData }
    }
}

impl<K, A: Fallible> Fallible for HashAdapter<K, A> { type Error = <A as Fallible>::Error; }
impl<K, A: Adapter> Adapter for HashAdapter<K, A>
    where K: Archive,
          Entry<K, A::Value>: Serialize<A::Serializer>,
{
    type Serializer = A::Serializer;
    type Value = Entry<K, A::Value>;

    fn new_serializer() -> Self::Serializer
        { A::new_serializer() }

    fn from_ser_err(e: <Self::Serializer as Fallible>::Error) -> <A as Fallible>::Error
        { A::from_ser_err(e) }

    fn from_db_err(e: lmdb::Error) -> <A as Fallible>::Error
        { A::from_db_err(e) }
}


const DEFAULT_HASH_FLAGS: libc::c_uint = 
    DatabaseFlags::INTEGER_KEY.bits() + DatabaseFlags::DUP_SORT.bits();

pub struct HashDB<A, K, H = RandomState>
{
    db: DB<HashAdapter<K, A>>,
    hash_builder: H,
}

impl<A, K> HashDB<A, K>
{
    pub unsafe fn create(env: &Environment, name: Option<&str>) -> lmdb::Result<Self> {
        Self::create_with_hasher(env, name, RandomState::new())
    }
    pub unsafe fn open(env: &Environment, name: Option<&str>) -> lmdb::Result<Self> {
        Self::open_with_hasher(env, name, RandomState::new())
    }
}

impl<A, K, H: BuildHasher> HashDB<A, K, H>
{
    fn new(db: DB<HashAdapter<K, A>>, hash_builder: H) -> Self {
        Self { db, hash_builder }
    }

    pub unsafe fn create_with_hasher(env: &Environment, name: Option<&str>, hash_builder: H)
        -> lmdb::Result<Self>
    {
        let flags = DatabaseFlags::from_bits(DEFAULT_HASH_FLAGS).unwrap();
        DB::create(env, name, flags).map(|db| Self::new(db, hash_builder))
    }
    pub unsafe fn open_with_hasher(env: &Environment, name: Option<&str>, hash_builder: H)
        -> lmdb::Result<Self>
    {
        DB::open(env, name).map(|db| Self::new(db, hash_builder))
    }

}

impl<A, K, H> HashDB<A, K, H>
    where A: Adapter,
          HashAdapter<K, A>: Adapter<Value=Entry<K, A::Value>>,
          H: BuildHasher,
          K: Hash + Archive,
          K::Archived: PartialEq<K>,
{
    /// Retrieve an entry from the hashdb
    ///
    /// The result is a view pinned to the lifetime of the transaction. You can get owned Values
    /// using [`Deserialize`].
    pub fn get<'txn, T: Transaction>(&self, txn: &'txn T, key: &K)
        -> Result<
            Option<&'txn Archived<<HashAdapter<K, A> as Adapter>::Value>>,
            <HashAdapter<K, A> as Fallible>::Error
            >
    {
        let mut hasher = self.hash_builder.build_hasher();
        key.hash(&mut hasher);
        let hash = hasher.finish();

        let mut cursor = self.db.open_ro_cursor(txn)?;
        let i = cursor
            .iter_dup_of(&hash.to_ne_bytes()).filter_map(|r| r.ok())
            .map(|(_keybuf, entry)| entry);
        for entry in i {
            let entry: &Archived<Entry<K, A::Value>> = entry;
            if entry.key == *key {
                return Ok(Some(entry));
            }
        }

        Ok(None)
    }
}

impl<'a, A, K, H> HashDB<A, K, H>
    where A: Adapter,
          A::Serializer: OutputBuffer,
          H: BuildHasher,
          K: Hash + Serialize<A::Serializer>,
          K::Archived: PartialEq<K>,
{
    pub fn insert_entry(&self, txn: &mut RwTransaction, entry: &Entry<K, A::Value>)
        -> Result<(), A::Error>
    {
        let mut hasher = self.hash_builder.build_hasher();
        entry.key.hash(&mut hasher);
        let hash = hasher.finish();

        self.db.put(txn, &hash.to_ne_bytes(), entry, WriteFlags::empty())?;

        Ok(())
    }
}
