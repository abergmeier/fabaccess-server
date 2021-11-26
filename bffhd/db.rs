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

use rkyv::{Fallible, Serialize, ser::serializers::AllocSerializer, AlignedVec, Archived};

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
    Entry,
};

mod fix;
pub use fix::LMDBorrow;

mod resources;
pub use resources::{
    ResourceDB,
};

mod pass;
pub use pass::{
    PassDB,
};

use lmdb::Error;
use rkyv::Deserialize;
use rkyv::ser::serializers::AlignedSerializer;
use std::sync::Arc;
use std::path::Path;
use crate::users::db::{User, UserDB};
use std::collections::HashMap;
use crate::resource::state::{OwnedEntry, State, db::StateDB};
use std::iter::FromIterator;
use std::ops::Deref;
use crate::utils::oid::{ArchivedObjectIdentifier, ObjectIdentifier};
use crate::resource::state::value::SerializeValue;

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

#[derive(Debug)]
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

#[derive(Debug, serde::Serialize)]
pub struct Dump {
    users: HashMap<String, User>,
    passwds: HashMap<String, String>,
    states: HashMap<String, (State, State)>,
}

impl Dump {
    pub fn new(dbs: &Databases) -> Result<Self> {
        let users = HashMap::from_iter(dbs.userdb.get_all()?.into_iter());
        let passwds = HashMap::from_iter(dbs.passdb.get_all()?.into_iter());
        let mut states = HashMap::new();
        for (name, id) in  dbs.resourcedb.get_all()?.into_iter() {
            let input = dbs.statedb.get_input(id)?.map(|input| {
                let input: &Archived<State> = input.deref();
                let hash: u64 = input.hash;
                let inner = input.inner.iter()
                    .map(|entry| {

                    let oid: &ArchivedObjectIdentifier = &entry.oid;
                    let bytes: &[u8] = oid.deref();
                    let mut vec = Vec::with_capacity(bytes.len());
                    vec.copy_from_slice(bytes);
                    let oid = ObjectIdentifier::new_unchecked(vec.into_boxed_slice());

                    let val: Box<dyn SerializeValue> = entry.val
                        .deserialize(&mut rkyv::Infallible).unwrap();

                    OwnedEntry { oid, val }
                }).collect();
                State { hash, inner }
            }).unwrap_or(State::build().finish());

            let output = dbs.statedb.get_output(id)?.map(|output| {
                let output: &Archived<State> = output.deref();
                let hash: u64 = output.hash;
                let inner = output.inner.iter().map(|entry| {

                    let oid: &ArchivedObjectIdentifier = &entry.oid;
                    let bytes: &[u8] = oid.deref();
                    let mut vec = Vec::with_capacity(bytes.len());
                    vec.copy_from_slice(bytes);
                    let oid = ObjectIdentifier::new_unchecked(vec.into_boxed_slice());

                    let val: Box<dyn SerializeValue> = entry.val
                        .deserialize(&mut rkyv::Infallible).unwrap();

                    OwnedEntry { oid, val }
                }).collect();

                State { hash, inner }
            }).unwrap_or(State::build().finish());

            let old = states.insert(name, (input, output));
            assert!(old.is_none());
        }

        Ok(Self { users, passwds, states })
    }
}