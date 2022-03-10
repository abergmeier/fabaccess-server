use std::{
    collections::hash_map::DefaultHasher,
    fmt,
    hash::{
        Hash,
        Hasher
    },
};
use std::fmt::Formatter;
use std::ops::Deref;

use rkyv::{
    Archive,
    Archived,
    Deserialize,
    out_field,
    Serialize,
};
use serde::de::{Error, MapAccess};
use serde::Deserializer;
use serde::ser::SerializeMap;

use value::{RegisteredImpl, SerializeValue};

use crate::utils::oid::ObjectIdentifier;
use crate::resources::state::value::{DynOwnedVal, DynVal, TypeOid, };

pub mod value;
pub mod db;

#[derive(serde::Serialize, serde::Deserialize)]
#[derive(Archive, Serialize, Deserialize)]
#[derive(Clone, PartialEq)]
#[archive_attr(derive(Debug))]
/// State object of a resources
///
/// This object serves three functions:
/// 1. it is constructed by modification via Claims or via internal resources logic
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

#[derive(Debug)]
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

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
#[archive_attr(derive(Debug))]
pub struct OwnedEntry {
    pub oid: ObjectIdentifier,
    pub val: Box<dyn SerializeValue>,
}

impl PartialEq for OwnedEntry {
    fn eq(&self, other: &Self) -> bool {
        self.oid == other.oid && self.val.dyn_eq(other.val.as_value())
    }
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

#[cfg(test)]
pub mod tests {
    use super::*;
    use super::value::*;

    pub(crate) fn gen_random() -> State {
        let amt: u8 = rand::random::<u8>() % 20;

        let mut sb = State::build();
        for _ in 0..amt {
            let oid = crate::utils::oid::tests::gen_random();
            sb = match rand::random::<u32>()%12 {
                0 => sb.add(oid, Box::new(rand::random::<bool>())),
                1 => sb.add(oid, Box::new(rand::random::<u8>())),
                2 => sb.add(oid, Box::new(rand::random::<u16>())),
                3 => sb.add(oid, Box::new(rand::random::<u32>())),
                4 => sb.add(oid, Box::new(rand::random::<u64>())),
                5 => sb.add(oid, Box::new(rand::random::<u128>())),
                6 => sb.add(oid, Box::new(rand::random::<i8>())),
                7 => sb.add(oid, Box::new(rand::random::<i16>())),
                8 => sb.add(oid, Box::new(rand::random::<i32>())),
                9 => sb.add(oid, Box::new(rand::random::<i64>())),
                10 => sb.add(oid, Box::new(rand::random::<i128>())),
                11 => sb.add(oid, Box::new(rand::random::<Vec3u8>())),
                _ => unreachable!(),
            }
        }
        sb.finish()
    }

    #[test]
    fn test_equal_state_is_eq() {
        let stateA = State::build()
            .add(OID_POWERED.clone(), Box::new(false))
            .add(OID_INTENSITY.clone(), Box::new(1024))
            .finish();

        let stateB = State::build()
            .add(OID_POWERED.clone(), Box::new(false))
            .add(OID_INTENSITY.clone(), Box::new(1024))
            .finish();

        assert_eq!(stateA, stateB);
    }

    #[test]
    fn test_unequal_state_is_ne() {
        let stateA = State::build()
            .add(OID_POWERED.clone(), Box::new(true))
            .add(OID_INTENSITY.clone(), Box::new(512))
            .finish();

        let stateB = State::build()
            .add(OID_POWERED.clone(), Box::new(false))
            .add(OID_INTENSITY.clone(), Box::new(1024))
            .finish();

        assert_ne!(stateA, stateB);
    }

    #[test]
    fn test_state_is_clone() {
        let stateA = gen_random();

        let stateB = stateA.clone();
        let stateC = stateB.clone();
        drop(stateA);

        assert_eq!(stateC, stateB);
    }
}