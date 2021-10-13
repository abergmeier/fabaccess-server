use std::{
    fmt,
    any::type_name,
    marker::PhantomData,
};

use rkyv::{
    Archived,
    archived_root,

    Serialize,

    ser::{
        Serializer,
        serializers::AllocSerializer,
    },

    util::AlignedVec,

    Fallible,
};

use lmdb::{
    Environment,
    DatabaseFlags,
    WriteFlags,

    Transaction,
    RwTransaction,
};

use super::RawDB;

pub trait Adapter: Fallible {
    type Serializer: rkyv::ser::Serializer;
    type Value: Serialize<Self::Serializer>;

    fn new_serializer() -> Self::Serializer;

    fn from_ser_err(e: <Self::Serializer as Fallible>::Error) -> <Self as Fallible>::Error;
    fn from_db_err(e: lmdb::Error) -> <Self as Fallible>::Error;
}

struct AdapterPrettyPrinter<A: Adapter>(PhantomData<A>);

impl<A: Adapter> AdapterPrettyPrinter<A> {
    pub fn new() -> Self { Self(PhantomData) }
}

impl<A: Adapter> fmt::Debug for AdapterPrettyPrinter<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(&type_name::<A>())
            .field("serializer", &type_name::<A::Serializer>())
            .field("value", &type_name::<A::Value>())
            .finish()
    }
}

pub trait OutputBuffer {
    type Buffer: AsRef<[u8]>;
    fn into_slice(self) -> Self::Buffer;
}

impl<const N: usize> OutputBuffer for AllocSerializer<N> {
    type Buffer = AlignedVec;
    fn into_slice(self) -> Self::Buffer {
        self.into_serializer().into_inner()
    }
}

pub trait OutputWriter: Fallible {
    fn write_into(&mut self, buf: &mut [u8]) -> Result<(), Self::Error>;
}

pub struct DB<A> {
    db: RawDB,
    phantom: PhantomData<A>,
}
impl<A> Clone for DB<A> {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
            phantom: PhantomData,
        }
    }
}
impl<A: Adapter> fmt::Debug for DB<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DB")
            .field("db", &self.db)
            .field("adapter", &AdapterPrettyPrinter::<A>::new())
            .finish()
    }
}

impl<A> DB<A> {
    fn new(db: RawDB) -> Self {
        Self { db, phantom: PhantomData }
    }

    /// Open the underlying DB, creating it if necessary
    ///
    /// This function is unsafe since if the DB does not contain `A::Archived` we may end up doing
    /// random memory reads or writes
    pub unsafe fn create(env: &Environment, name: Option<&str>, flags: DatabaseFlags)
        -> lmdb::Result<Self> 
    {
        RawDB::create(env, name, flags).map(Self::new)
    }

    /// Open the underlying DB
    ///
    /// This function is unsafe since if the DB does not contain `A::Archived` we may end up doing
    /// random memory reads or writes
    pub unsafe fn open(env: &Environment, name: Option<&str>) -> lmdb::Result<Self> {
        RawDB::open(env, name).map(Self::new)
    }
}

impl<A: Adapter> DB<A>
{
    pub fn del<K: AsRef<[u8]>>(&self, txn: &mut RwTransaction, key: &K) -> Result<(), A::Error> {
        let v: Option<&Vec<u8>> = None;
        self.db.del(txn, key, v).map_err(A::from_db_err)
    }
}

impl<A: Adapter> DB<A>
{
    pub fn get<'txn, T: Transaction, K: AsRef<[u8]>>(&self, txn: &'txn T, key: &K) 
        -> Result<Option<&'txn Archived<A::Value>>, A::Error>
    {
        if let Some(buf) = self.db.get(txn, key).map_err(A::from_db_err)? {
            Ok(Some(unsafe { archived_root::<A::Value>(buf.as_ref()) }))
        } else {
            Ok(None)
        }
    }

    pub fn open_ro_cursor<'txn, T: Transaction>(&self, txn: &'txn T) 
        -> Result<Cursor<lmdb::RoCursor<'txn>, A>, A::Error>
    {
        let c = self.db.open_ro_cursor(txn)
            .map_err(A::from_db_err)?;
        // Safe because we are providing both Adapter and cursor and know it matches
        Ok(unsafe { Cursor::new(c) })
    }
}

impl<'a, A> DB<A>
    where A: Adapter,
          A::Serializer: OutputBuffer,
{
    pub fn put<K: AsRef<[u8]>>(&self, txn: &mut RwTransaction, key: &K, val: &A::Value, flags: WriteFlags) 
        -> Result<usize, A::Error>
    {
        let mut serializer = A::new_serializer();
        let pos = serializer.serialize_value(val)
            .map_err(A::from_ser_err)?;

        let buf = serializer.into_slice();
        self.db.put(txn, key, &buf, flags)
            .map_err(A::from_db_err)?;

        Ok(pos)
    }
}

impl<'a, A> DB<A>
    where A: Adapter,
          A::Serializer: OutputWriter,
{
    pub fn put_nocopy<K: AsRef<[u8]>>(&self, txn: &mut RwTransaction, key: &K, val: &A::Value, flags: WriteFlags)
        -> Result<usize, A::Error>
    {
        let mut serializer = A::new_serializer();
        let pos = serializer.serialize_value(val)
            .map_err(A::from_ser_err)?;

        let mut buf = self.db.reserve(txn, &key.as_ref(), pos, flags)
            .map_err(A::from_db_err)?;
        serializer.write_into(&mut buf)
            .map_err(A::from_ser_err)?;

        Ok(pos)
    }
}

pub struct Cursor<C, A> {
    cursor: C,
    phantom: PhantomData<A>,
}

impl<'txn, C, A> Cursor<C, A>
    where C: lmdb::Cursor<'txn>,
          A: Adapter,
{
    // Unsafe because we don't know if the given adapter matches the given cursor
    pub unsafe fn new(cursor: C) -> Self {
        Self { cursor, phantom: PhantomData }
    }

    pub fn iter_start(&mut self) -> Iter<'txn, A> {
        let iter = self.cursor.iter_start();
        // Safe because `new` isn't :P
        unsafe { Iter::new(iter) }
    }

    pub fn iter_dup_of<K: AsRef<[u8]>>(&mut self, key: &K) -> Iter<'txn, A> {
        let iter = self.cursor.iter_dup_of(key);
        // Safe because `new` isn't :P
        unsafe { Iter::new(iter) }
    }
}

pub struct Iter<'txn, A> {
    iter: lmdb::Iter<'txn>,
    phantom: PhantomData<A>,
}

impl<'txn, A: Adapter> Iter<'txn, A> {
    pub unsafe fn new(iter: lmdb::Iter<'txn>) -> Self {
        Self { iter, phantom: PhantomData }
    }
}

impl<'txn, A: Adapter> Iterator for Iter<'txn, A> 
    where Archived<A::Value>: 'txn
{
    type Item = Result<(&'txn [u8], &'txn Archived<A::Value>), A::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|r| r
            .map_err(A::from_db_err)
            .map(|(key, buf)| { (key, unsafe { archived_root::<A::Value>(buf) }) }))
    }
}
