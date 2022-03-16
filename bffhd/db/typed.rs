use crate::db::RawDB;
use lmdb::{Cursor, RwTransaction, Transaction, WriteFlags};
use rkyv::{AlignedVec, Archive, Archived, Serialize};
use std::fmt;
use std::fmt::{Debug, Display, Formatter};
use std::marker::PhantomData;
use std::pin::Pin;
use crate::db;

#[derive(Clone)]
/// Packed, sendable resource state
pub struct ArchivedValue<T> {
    /// State is encoded using rkyv making it trivially serializable
    data: AlignedVec,
    _marker: PhantomData<T>,
}
impl<T> ArchivedValue<T> {
    pub fn new(data: AlignedVec) -> Self {
        Self {
            data,
            _marker: PhantomData,
        }
    }
    pub fn build(data: &[u8]) -> Self {
        let mut v = AlignedVec::with_capacity(data.len());
        v.extend_from_slice(data);
        Self::new(v)
    }

    pub fn as_mut(&mut self) -> &mut AlignedVec {
        &mut self.data
    }

    pub fn as_slice(&self) -> &[u8] {
        self.data.as_slice()
    }
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        self.data.as_mut_slice()
    }
}
impl<T: Archive> AsRef<Archived<T>> for ArchivedValue<T> {
    fn as_ref(&self) -> &Archived<T> {
        unsafe { rkyv::archived_root::<T>(self.as_slice()) }
    }
}

//
// Debug implementation shows wrapping SendState
//
impl<T: Archive> Debug for ArchivedValue<T>
where
    <T as Archive>::Archived: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SendState").field(self.as_ref()).finish()
    }
}

//
// Display implementation hides wrapping SendState
//
impl<T: Archive> Display for ArchivedValue<T>
where
    <T as Archive>::Archived: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self.as_ref(), f)
    }
}

/// Adapter trait handling de-/serialization
///
/// Values must be read from raw, unaligned byte buffers provided by LMDB.
pub trait Adapter {
    type Item;

    /// Decode data from a short-lived byte buffer into a durable format
    fn decode(data: &[u8]) -> Self::Item;

    fn encoded_len(item: &Self::Item) -> usize;
    fn encode_into(item: &Self::Item, buf: &mut [u8]);
}

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct AlignedAdapter<V>(PhantomData<V>);
impl<V> Adapter for AlignedAdapter<V> {
    type Item = ArchivedValue<V>;

    fn decode(data: &[u8]) -> Self::Item {
        ArchivedValue::build(data)
    }

    fn encoded_len(item: &Self::Item) -> usize {
        item.as_slice().len()
    }

    fn encode_into(item: &Self::Item, buf: &mut [u8]) {
        buf.copy_from_slice(item.as_slice())
    }
}

#[derive(Debug, Clone)]
#[repr(transparent)]
/// `Typed` database, allowing storing a typed value
///
/// Values must be serialized into and deserialized from raw byte buffers.
/// This is handled by a stateless [Adapter] given by the type parameter `A`
pub struct DB<A> {
    db: RawDB,
    _marker: PhantomData<A>,
}
impl<A> DB<A> {
    pub fn new(db: RawDB) -> Self {
        Self {
            db,
            _marker: PhantomData,
        }
    }
}

impl<A: Adapter> DB<A> {
    pub fn get<T: Transaction>(&self, txn: &T, key: &impl AsRef<[u8]>) -> Result<Option<A::Item>, db::Error> {
        Ok(self.db.get(txn, key)?.map(A::decode))
    }

    pub fn put(
        &self,
        txn: &mut RwTransaction,
        key: &impl AsRef<[u8]>,
        value: &A::Item,
        flags: WriteFlags,
    ) -> Result<(), db::Error>
    {
        let len = A::encoded_len(value);
        let buf = self.db.reserve(txn, key, len, flags)?;
        assert_eq!(buf.len(), len, "Reserved buffer is not of requested size!");
        A::encode_into(value, buf);
        Ok(())
    }

    pub fn del(&self, txn: &mut RwTransaction, key: &impl AsRef<[u8]>) -> Result<(), db::Error> {
        self.db.del::<_, &[u8]>(txn, key, None)
    }

    pub fn get_all<'txn, T: Transaction>(&self, txn: &'txn T) -> Result<impl IntoIterator<Item=(&'txn [u8], A::Item)>, db::Error> {
        let mut cursor = self.db.open_ro_cursor(txn)?;
        let it = cursor.iter_start();
        Ok(it.filter_map(|buf| buf.ok().map(|(kbuf,vbuf)| {
            (kbuf, A::decode(vbuf))
        })))
    }
}