use std::{
    marker::PhantomData,
};

pub use lmdb::{
    Environment,

    DatabaseFlags,
    WriteFlags,
    EnvironmentFlags,

    Transaction,
    RoTransaction,
    RwTransaction,
};

use rkyv::{Fallible, Serialize, ser::serializers::AllocSerializer, AlignedVec};

mod raw;
use raw::RawDB;

mod typed;
// re-exports
pub use typed::{
    DB,
    TypedCursor,

    Adapter,
    OutputBuffer,
    OutputWriter,
};

mod hash;
pub use hash::{
    HashDB,
    Entry,
};

mod fix;
pub use fix::LMDBorrow;

pub mod state;
pub use state::{
    StateDB,
};

mod resources;
pub use resources::{
    ResourceDB,
};

mod pass;
pub use pass::{
    PassDB,
};

mod user;
pub use user::{
    UserDB,
};

use lmdb::Error;
use rkyv::ser::serializers::AlignedSerializer;
use std::sync::Arc;
use std::path::Path;
use crate::db::user::User;
use crate::db::resources::Resource;
use std::collections::HashMap;
use crate::state::State;
use std::iter::FromIterator;

#[derive(Debug)]
pub enum DBError {
    LMDB(lmdb::Error),
    RKYV(<AllocSerializer<1024> as Fallible>::Error),
}

pub(crate) type Result<T> = std::result::Result<T, DBError>;

impl From<lmdb::Error> for DBError {
    fn from(e: lmdb::Error) -> Self {
        Self::LMDB(e)
    }
}

type Ser = AllocSerializer<1024>;
#[derive(Clone)]
struct AllocAdapter<V> {
    phantom: PhantomData<V>,
}

impl<V> Fallible for AllocAdapter<V> {
    type Error = DBError;
}

impl<V: Serialize<Ser>> Adapter for AllocAdapter<V> {
    type Serializer = Ser;
    type Value = V;

    fn new_serializer() -> Self::Serializer {
        Self::Serializer::default()
    }

    fn from_ser_err(e: <Self::Serializer as Fallible>::Error) -> Self::Error {
        DBError::RKYV(e)
    }
    fn from_db_err(e: lmdb::Error) -> Self::Error {
        e.into()
    }
}

#[derive(Copy, Clone)]
pub struct AlignedAdapter<V> {
    phantom: PhantomData<V>,
}
impl<V> Fallible for AlignedAdapter<V> {
    type Error = lmdb::Error;
}
impl<V: Serialize<AlignedSerializer<AlignedVec>>> Adapter for AlignedAdapter<V> {
    type Serializer = AlignedSerializer<AlignedVec>;
    type Value = V;

    fn new_serializer() -> Self::Serializer {
        Self::Serializer::default()
    }

    fn from_ser_err(_: <Self::Serializer as Fallible>::Error) -> <Self as Fallible>::Error {
        unreachable!()
    }

    fn from_db_err(e: Error) -> <Self as Fallible>::Error {
        e
    }
}

pub struct Databases {
    pub userdb: UserDB,
    pub passdb: PassDB,
    pub resourcedb: ResourceDB,
    pub statedb: StateDB,
}

impl Databases {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let env = Arc::new(Environment::new()
            .open(&Path::join(path.as_ref(), "internal"))?
        );
        let userdb = unsafe { UserDB::open(env.clone())? };
        let passdb = unsafe { PassDB::open(env.clone())? };
        let resourcedb = unsafe { ResourceDB::open(env)? };

        let statedb = StateDB::open(&Path::join(path.as_ref(), "state"))?;

        Ok(Self { userdb, passdb, resourcedb, statedb })
    }

    pub fn create<P: AsRef<Path>>(path: P) -> Result<Self> {
        let env = Arc::new(Environment::new()
            .set_max_dbs(16)
            .open(path.as_ref())?
        );
        let userdb = unsafe { UserDB::create(env.clone())? };
        let passdb = unsafe { PassDB::create(env.clone())? };
        let resourcedb = unsafe { ResourceDB::create(env)? };

        let statedb = StateDB::create(&Path::join(path.as_ref(), "state"))?;

        Ok(Self { userdb, passdb, resourcedb, statedb })
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Dump {
    users: HashMap<String, User>,
    passwds: HashMap<String, String>,
    resources: HashMap<String, Resource>,
    states: HashMap<String, (State, State)>,
}

impl Dump {
    pub fn new(dbs: &Databases) -> Result<Self> {
        let users = HashMap::from_iter(dbs.userdb.get_all()?.into_iter());
        let passwds = HashMap::new();
        let resources = HashMap::new();
        let states = HashMap::new();

        Ok(Self { users, passwds, resources, states })
    }
}