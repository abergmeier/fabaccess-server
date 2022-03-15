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