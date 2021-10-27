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

pub mod value;
use value::{SerializeValue, RegisteredImpl};
use crate::state::value::{TypeOid, DynVal, DynOwnedVal, };
use crate::oid::ObjectIdentifier;
use serde::ser::SerializeMap;
use std::ops::Deref;
use std::fmt::Formatter;
use serde::Deserializer;
use serde::de::MapAccess;
use serde::de::Error as _;

#[derive(serde::Serialize, serde::Deserialize)]
#[derive(Archive, Serialize, Deserialize)]
#[archive_attr(derive(Debug))]
/// State object of a resource
///
/// This object serves three functions:
/// 1. it is constructed by modification via Claims or via internal resource logic
/// 2. it is serializable and storable in the database
/// 3. it is sendable and forwarded to all Actors and Notifys
pub struct State {
    pub hash: u64,
    pub inner: Vec<OwnedEntry>,
}

impl State {
    pub fn build() -> StateBuilder {
        StateBuilder::new()
    }
    pub fn hash(&self) -> u64 {
        self.hash
    }
}

impl PartialEq for State {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl PartialEq<Archived<State>> for State {
    fn eq(&self, other: &Archived<Self>) -> bool {
        self.hash == other.hash
    }
}

impl Eq for State {}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut sf = f.debug_struct("State");
        for OwnedEntry { oid, val } in self.inner.iter() {
            let k: String = oid.into();
            sf.field(k.as_ref(), val);
        }
        sf.finish()
    }
}

pub struct StateBuilder {
    hasher: DefaultHasher,
    inner: Vec<OwnedEntry>
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
    pub fn add<V>(mut self, oid: ObjectIdentifier, val: Box<V>) -> Self
        where V: SerializeValue + Hash + Archive,
              Archived<V>: TypeOid + RegisteredImpl,
    {
    // Hash before creating the StateEntry struct which removes the type information
        oid.hash(&mut self.hasher);
        val.hash(&mut self.hasher);
        self.inner.push(OwnedEntry { oid, val });

        self
    }
}

#[derive(Debug)]
pub struct Entry<'a> {
    pub oid: &'a ObjectIdentifier,
    pub val: &'a dyn SerializeValue,
}

#[derive(Debug, Archive, Serialize, Deserialize)]
#[archive_attr(derive(Debug))]
pub struct OwnedEntry {
    pub oid: ObjectIdentifier,
    pub val: Box<dyn SerializeValue>,
}

impl<'a> serde::Serialize for Entry<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: serde::Serializer
    {
        let mut ser = serializer.serialize_map(Some(1))?;
        ser.serialize_entry(&self.oid, &DynVal(self.val))?;
        ser.end()
    }
}

impl serde::Serialize for OwnedEntry {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: serde::Serializer
    {
        let mut ser = serializer.serialize_map(Some(1))?;
        ser.serialize_entry(&self.oid, &DynVal(self.val.deref()))?;
        ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for OwnedEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        deserializer.deserialize_map(OwnedEntryVisitor)
    }
}

struct OwnedEntryVisitor;
impl<'de> serde::de::Visitor<'de> for OwnedEntryVisitor {
    type Value = OwnedEntry;

    fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "an one entry map from OID to some value object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error>
    {
        let oid: ObjectIdentifier = map.next_key()?
            .ok_or(A::Error::missing_field("oid"))?;
        let val: DynOwnedVal = map.next_value()?;
        Ok(OwnedEntry { oid, val: val.0 })
    }
}
