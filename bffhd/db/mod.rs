use std::marker::PhantomData;

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
pub use raw::RawDB;

mod typed;
// re-exports
pub use typed::{
    DB,
    TypedCursor,

    Adapter,
    OutputBuffer,
};

mod hash;
pub use hash::{
    HashDB,
};

mod fix;

pub mod index;
pub use fix::LMDBorrow;

use lmdb::Error;
use rkyv::Deserialize;
use rkyv::ser::serializers::AlignedSerializer;


use crate::users::db::{User};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use rkyv::Infallible;
use crate::resources::state::{State};
use std::iter::FromIterator;
use std::ops::Deref;
use crate::resources::search::ResourcesHandle;


use crate::Users;

#[derive(Debug)]
pub enum DBError {
    LMDB(lmdb::Error),
    RKYV(<AllocSerializer<1024> as Fallible>::Error),
}
impl Display for DBError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LMDB(e) => write!(f, "LMDB error: {}", e),
            Self::RKYV(e) => write!(f, "rkyv error: {}", e),
        }
    }
}
impl std::error::Error for DBError { }

pub(crate) type Result<T> = std::result::Result<T, DBError>;

impl From<lmdb::Error> for DBError {
    fn from(e: lmdb::Error) -> Self {
        Self::LMDB(e)
    }
}

type Ser = AllocSerializer<1024>;
#[derive(Clone)]
pub struct AllocAdapter<V> {
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

#[derive(Copy, Clone, Debug)]
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

#[derive(Debug, serde::Serialize)]
pub struct Dump {
    users: HashMap<String, User>,
    states: HashMap<String, State>,
}

impl Dump {
    pub fn new(userdb: Users, resources: ResourcesHandle) -> Result<Self> {
        let users = HashMap::from_iter(userdb.into_inner().get_all()?.into_iter());
        let mut states = HashMap::new();
        for resource in resources.list_all().into_iter() {
            if let Some(output) = resource.get_raw_state() {
                let output: State = Deserialize::<State, _>::deserialize(output.deref(), &mut Infallible).unwrap();
                let old = states.insert(resource.get_id().to_string(), output);
                assert!(old.is_none());
            }
        }

        Ok(Self { users, states })
    }
}