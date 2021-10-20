use std::{
    ptr::NonNull,
    ops::Deref,
};

use crate::db::Transaction;
use std::fmt::{Debug, Formatter};

/// Memory Fixpoint for a value in the DB
///
/// LMDB binds lifetimes of buffers to the transaction that returned the buffer. As long as this
/// transaction is not `commit()`ed, `abort()`ed or `reset()`ed the pages containing these values
/// are not returned into circulation.
/// This struct encodes this by binding a live reference to the Transaction to the returned
/// and interpreted buffer. The placeholder `T` is the container for the transaction. This may be a
/// plain `RoTransaction<'env>`, a `Rc<RoTxn>` (meaning Fix is !Send) or an `Arc<RoTxn>`, depending
/// on your needs.
pub struct LMDBorrow<T, V> {
    ptr: NonNull<V>,
    txn: T,
}

impl<'env, T, V> LMDBorrow<T, V>
    where T: Transaction,
{
    pub unsafe fn new(ptr: NonNull<V>, txn: T) -> Self {
        Self { ptr: ptr.into(), txn }
    }

    pub fn unwrap_txn(self) -> T {
        self.txn
    }
}

impl<'env, T, V> Deref for LMDBorrow<T, V>
{
    type Target = V;

    fn deref(&self) -> &Self::Target {
        // As long as the transaction is kept alive (which it is, because it's in self) state is a
        // valid pointer so this is safe.
        unsafe { self.ptr.as_ref() }
    }
}

impl<'env, T, V: Debug> Debug for LMDBorrow<T, V> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.deref())
    }
}

impl<'env, T, V: serde::Serialize> serde::Serialize for LMDBorrow<T, V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: serde::Serializer
    {
        self.deref().serialize(serializer)
    }
}