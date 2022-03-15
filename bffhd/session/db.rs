use std::sync::Arc;
use lmdb::{DatabaseFlags, Environment};

use rkyv::{Archive, Serialize, Deserialize};

use crate::db::{AllocAdapter, DB, RawDB};
use crate::users::UserRef;

#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Archive, Serialize, Deserialize)]
pub struct Session {
   userid: UserRef,
}

type Adapter = AllocAdapter<Session>;
pub struct SessionCache {
    env: Arc<Environment>,
    db: DB<Adapter>,
}

impl SessionCache {
    pub unsafe fn new(env: Arc<Environment>, db: RawDB) -> Self {
        let db = DB::new_unchecked(db);
        Self { env, db }
    }

    pub unsafe fn open(env: Arc<Environment>) -> lmdb::Result<Self> {
        let db = RawDB::open(&env, Some("sessions"))?;
        Ok(Self::new(env, db))
    }

    pub unsafe fn create(env: Arc<Environment>) -> lmdb::Result<Self> {
        let flags = DatabaseFlags::empty();
        let db = RawDB::create(&env, Some("sessions"), flags)?;
        Ok(Self::new(env, db))
    }
}