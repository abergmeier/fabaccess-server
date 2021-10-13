use std::{
    fmt,

    collections::{
        hash_map::DefaultHasher
    },
    hash::{
        Hash,
        Hasher
    },
};

use rkyv::{
    Archive,
    Archived,

    Serialize,
    Deserialize,

    out_field,
};

use crate::oid::ObjectIdentifier;

pub mod value;
pub use value::{
    SerializeDynValue,
    DeserializeDynValue,
};

#[derive(Archive, Serialize, Deserialize)]
pub struct StateEntry {
    key: ObjectIdentifier,
    val: Box<dyn SerializeDynValue>,
}

#[derive(Archive, Serialize, Deserialize)]
/// State object of a resource
///
/// This object serves three functions:
/// 1. it is constructed by modification via Claims or via internal resource logic
/// 2. it is serializable and storable in the database
/// 3. it is sendable and forwarded to all Actors and Notifys
pub struct State {
    hash: u64,
    inner: Vec<StateEntry>,
}
impl PartialEq for State {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}
impl PartialEq<Archived<State>> for State {
    fn eq(&self, other: &Archived<State>) -> bool {
        self.hash == other.hash
    }
}
impl Eq for State {}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut sf = f.debug_struct("State");
        for StateEntry { key, val } in self.inner.iter() {
            let k: String = key.into();
            sf.field(k.as_ref(), val);
        }
        sf.finish()
    }
}

impl State {
    pub fn build() -> StateBuilder {
        StateBuilder::new()
    }

    pub fn hash(&self) -> u64 {
        self.hash
    }
}

pub struct StateBuilder {
    hasher: DefaultHasher,
    inner: Vec<StateEntry>
}

impl StateBuilder {
    pub fn new() -> Self {
        let hasher = DefaultHasher::new();
        Self { inner: Vec::new(), hasher }
    }

    pub fn finish(self) -> State {
        State {
            hash: self.hasher.finish(),
            inner: self.inner,
        }
    }

    /// Add key-value pair to the State being built.
    ///
    /// We have to use this split system here because type erasure prevents us from limiting values
    /// to `Hash`. Specifically, you can't have a trait object of `Hash` because `Hash` depends on
    /// `Self`. In this function however the compiler still knows the exact type of `V` and can
    /// call statically call its `hash` method.
    pub fn add<V>(mut self, key: ObjectIdentifier<>, val: Box<V>) -> Self
        where V: SerializeDynValue + Hash
    {
    // Hash before creating the StateEntry struct which removes the type information
        key.hash(&mut self.hasher);
        val.hash(&mut self.hasher);
        self.inner.push(StateEntry { key, val });

        self
    }
}