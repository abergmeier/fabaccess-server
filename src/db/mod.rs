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

use rkyv::{
    Fallible,
    Serialize,
    ser::serializers::AllocSerializer,
};

mod raw;
use raw::RawDB;

mod typed;
// re-exports
pub use typed::{
    DB,
    Cursor,

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

pub enum DBError {
    LMDB(lmdb::Error),
    RKYV(<AllocSerializer<1024> as Fallible>::Error),
}

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

