use std::fmt::{Debug, Display, Formatter};
use std::{fmt, hash::Hasher};

use std::ops::Deref;

use rkyv::{out_field, Archive, Deserialize, Serialize};
use serde::de::{Error, MapAccess, Unexpected};
use serde::ser::SerializeMap;
use serde::Deserializer;

use crate::resources::modules::fabaccess::OID_VALUE;
use crate::MachineState;

use crate::utils::oid::ObjectIdentifier;

pub mod db;
pub mod value;

#[derive(Archive, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[archive_attr(derive(Debug))]
pub struct State {
    pub inner: MachineState,
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut sf = f.debug_struct("State");
        //for Entry { oid, val } in self.inner.iter() {
        let k: String = OID_VALUE.deref().into();
        sf.field(k.as_ref(), &self.inner);
        //}
        sf.finish()
    }
}

impl fmt::Display for ArchivedState {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl serde::Serialize for State {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut ser = serializer.serialize_map(Some(1))?;
        ser.serialize_entry(OID_VALUE.deref(), &self.inner)?;
        ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for State {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(StateVisitor)
    }
}

struct StateVisitor;
impl<'de> serde::de::Visitor<'de> for StateVisitor {
    type Value = State;

    fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "a map from OIDs to value objects")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let oid: ObjectIdentifier = map.next_key()?.ok_or(A::Error::missing_field("oid"))?;
        if oid != *OID_VALUE.deref() {
            return Err(A::Error::invalid_value(
                Unexpected::Other("Unknown OID"),
                &"OID of fabaccess state",
            ));
        }
        let val: MachineState = map.next_value()?;
        Ok(State { inner: val })
    }
}

#[cfg(test)]
pub mod tests {
    use super::value::*;
    use super::*;

    pub(crate) fn gen_random() -> State {
        let amt: u8 = rand::random::<u8>() % 20;

        let mut sb = State::build();
        for _ in 0..amt {
            let oid = crate::utils::oid::tests::gen_random();
            sb = match rand::random::<u32>() % 12 {
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
