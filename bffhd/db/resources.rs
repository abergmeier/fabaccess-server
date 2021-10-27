use rkyv::{Archive, Serialize, Deserialize};

use super::{
    DB,
};
use crate::db::{AlignedAdapter, AllocAdapter};
use crate::db::raw::RawDB;
use std::sync::Arc;
use crate::db::{Environment, DatabaseFlags};
use crate::db::Result;

#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Archive, Serialize, Deserialize)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Resource {
    uuid: u128,
    id: String,
    name_idx: u64,
    description_idx: u64,
}

#[derive(Debug, Clone)]
pub struct ResourceDB {
    env: Arc<Environment>,
    db: DB<AllocAdapter<Resource>>,
    id_index: DB<AlignedAdapter<u64>>,
}

impl ResourceDB {
    pub unsafe fn new(env: Arc<Environment>, db: RawDB, id_index: RawDB) -> Self {
        let db = DB::new_unchecked(db);
        let id_index = DB::new_unchecked(id_index);

        Self { env, db, id_index }
    }

    pub unsafe fn open(env: Arc<Environment>) -> Result<Self> {
        let db = RawDB::open(&env, Some("resources"))?;
        let idx = RawDB::open(&env, Some("resources-idx"))?;
        Ok(Self::new(env, db, idx))
    }

    pub unsafe fn create(env: Arc<Environment>) -> Result<Self> {
        let flags = DatabaseFlags::empty();
        let db = RawDB::create(&env, Some("resources"), flags)?;
        let idx = RawDB::create(&env, Some("resources-idx"), flags)?;
        Ok(Self::new(env, db, idx))
    }

    pub fn lookup_id<S: AsRef<str>>(&self, id: S) -> Result<Option<u64>> {
        let txn = self.env.begin_ro_txn()?;
        let id = self.id_index.get(&txn, &id.as_ref().as_bytes()).map(|ok| {
            ok.map(|num| *num)
        })?;
        Ok(id)
    }

    pub fn get_all(&self) -> Result<Vec<(String, u64)>> {
        let txn = self.env.begin_ro_txn()?;
        let mut cursor = self.id_index.open_ro_cursor(&txn)?;
        let iter = cursor.iter_start();
        let mut out = Vec::new();

        for id in iter {
            let (name, id) = id?;
            let name = unsafe { std::str::from_utf8_unchecked(name).to_string() };
            out.push((name, *id));
        }

        Ok(out)
    }
}