use rkyv::{Archive, Serialize, Deserialize};

use super::{
    AllocAdapter,
    DB,
};
use crate::db::AlignedAdapter;
use crate::db::raw::RawDB;
use std::sync::Arc;
use lmdb::Environment;
use crate::db;

#[derive(Archive, Serialize, Deserialize)]
pub struct Resource {
    uuid: u128,
    id: String,
    name_idx: u64,
    description_idx: u64,
}

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

    pub fn lookup_id<S: AsRef<str>>(&self, id: S) -> Result<Option<u64>, db::Error> {
        let txn = self.env.begin_ro_txn()?;
        self.id_index.get(&txn, &id.as_ref().as_bytes()).map(|ok| {
            ok.map(|num| *num)
        })
    }
}