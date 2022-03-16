use lmdb::{
    Transaction,
    RwTransaction,
    Environment,
    DatabaseFlags,
    WriteFlags,
};

#[derive(Debug, Clone)]
pub struct RawDB {
    db: lmdb::Database,
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

    pub fn iter<'txn, C: lmdb::Cursor<'txn>>(&self, cursor: &'txn mut C) -> lmdb::Iter<'txn> {
        cursor.iter_start()
    }

    pub fn open_ro_cursor<'txn, T: Transaction>(&self, txn: &'txn T) -> lmdb::Result<lmdb::RoCursor<'txn>> {
        txn.open_ro_cursor(self.db)
    }
}
