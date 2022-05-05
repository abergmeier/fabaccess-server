//! oid crate by <https://github.com/UnnecessaryEngineering/oid> turned into vendored module
//!
//! [Object Identifiers] are a standard of the [ITU] used to reference objects, things, and
//! concepts in a globally unique way. This crate provides for data structures and methods
//! to build, parse, and format OIDs.
//!
//!
//! ## Parsing OID String Representation
//! ```ignore
//! use crate::oid::prelude::*;
//!
//! fn main() -> Result<(), ObjectIdentifierError> {
//!     let oid = ObjectIdentifier::try_from("0.1.2.3")?;
//!     Ok(())
//! }
//! ```
//!
//! ## Parsing OID Binary Representation
//! ```ignore
//! use prelude::*;
//!
//! fn main() -> Result<(), ObjectIdentifierError> {
//!     let oid = ObjectIdentifier::try_from(vec![0x00, 0x01, 0x02, 0x03])?;
//!     Ok(())
//! }
//! ```
//!
//! ## Encoding OID as String Representation
//! ```ignore
//! use prelude::*;
//!
//! fn main() -> Result<(), ObjectIdentifierError> {
//!     let oid = ObjectIdentifier::try_from("0.1.2.3")?;
//!     let oid: String = oid.into();
//!     assert_eq!(oid, "0.1.2.3");
//!     Ok(())
//! }
//! ```
//!
//! ## Encoding OID as Binary Representation
//! ```ignore
//! use oid::prelude::*;
//!
//! fn main() -> Result<(), ObjectIdentifierError> {
//!     let oid = ObjectIdentifier::try_from(vec![0x00, 0x01, 0x02, 0x03])?;
//!     let oid: Vec<u8> = oid.into();
//!     assert_eq!(oid, vec![0x00, 0x01, 0x02, 0x03]);
//!     Ok(())
//! }
//! ```
//!
//! [Object Identifiers]: https://en.wikipedia.org/wiki/Object_identifier
//! [ITU]: https://en.wikipedia.org/wiki/International_Telecommunications_Union

use crate::utils::varint::VarU128;
use rkyv::ser::Serializer;
use rkyv::vec::{ArchivedVec, VecResolver};
use rkyv::{Archive, Serialize};
use std::convert::TryFrom;
use std::convert::TryInto;
use std::fmt;
use std::fmt::Formatter;
use std::ops::Deref;
use std::str::FromStr;

type Node = u128;
type VarNode = VarU128;

/// Convenience module for quickly importing the public interface (e.g., `use oid::prelude::*`)
pub mod prelude {
    pub use super::ObjectIdentifier;
    pub use super::ObjectIdentifierError;
    pub use super::ObjectIdentifierRoot::*;
    pub use core::convert::{TryFrom, TryInto};
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[repr(u8)]
pub enum ObjectIdentifierRoot {
    ItuT = 0,
    Iso = 1,
    JointIsoItuT = 2,
}

impl Into<String> for ObjectIdentifierRoot {
    fn into(self) -> String {
        format!("{}", self as u8)
    }
}

impl TryFrom<u8> for ObjectIdentifierRoot {
    type Error = ObjectIdentifierError;
    fn try_from(value: u8) -> Result<ObjectIdentifierRoot, Self::Error> {
        match value {
            0 => Ok(ObjectIdentifierRoot::ItuT),
            1 => Ok(ObjectIdentifierRoot::Iso),
            2 => Ok(ObjectIdentifierRoot::JointIsoItuT),
            _ => Err(ObjectIdentifierError::IllegalRootNode),
        }
    }
}

/// Object Identifier Errors
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ObjectIdentifierError {
    /// Failed to parse OID due to illegal root node (must be 0-2 decimal)
    IllegalRootNode,
    /// Failed to parse OID due to illegal first node (must be 0-39 decimal)
    IllegalFirstChildNode,
    /// Failed to parse OID due to illegal child node value (except first node)
    IllegalChildNodeValue,
}

/// Object Identifier (OID)
#[derive(Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct ObjectIdentifier {
    nodes: Box<[u8]>,
}

impl ObjectIdentifier {
    #[inline(always)]
    pub const fn new_unchecked(nodes: Box<[u8]>) -> Self {
        Self { nodes }
    }
    pub fn from_box(nodes: Box<[u8]>) -> Result<Self, ObjectIdentifierError> {
        if nodes.len() < 1 {
            return Err(ObjectIdentifierError::IllegalRootNode);
        };
        ObjectIdentifierRoot::try_from(nodes[0] / 40)?;

        let mut parsing_big_int = false;
        let mut big_int: Node = 0;
        for i in 1..nodes.len() {
            if !parsing_big_int && nodes[i] < 128 {
            } else {
                if big_int > 0 {
                    if big_int >= Node::MAX >> 7 {
                        return Err(ObjectIdentifierError::IllegalChildNodeValue);
                    }
                    big_int <<= 7;
                };
                big_int |= (nodes[i] & !0x80) as Node;
                parsing_big_int = nodes[i] & 0x80 != 0;
            }
            if big_int > 0 && !parsing_big_int {
                big_int = 0;
            }
        }
        Ok(Self { nodes })
    }

    pub fn build<B: AsRef<[Node]>>(
        root: ObjectIdentifierRoot,
        first: u8,
        children: B,
    ) -> Result<Self, ObjectIdentifierError> {
        if first > 40 {
            return Err(ObjectIdentifierError::IllegalFirstChildNode);
        }

        let children = children.as_ref();
        let mut vec = Vec::with_capacity(children.len() + 1);
        vec.push((root as u8) * 40 + first);
        for child in children {
            let var: VarNode = child.into();
            vec.extend_from_slice(var.as_bytes())
        }
        Ok(Self {
            nodes: vec.into_boxed_slice(),
        })
    }

    #[inline(always)]
    pub fn root(&self) -> Result<ObjectIdentifierRoot, ObjectIdentifierError> {
        ObjectIdentifierRoot::try_from(self.nodes[0] / 40)
    }
    #[inline(always)]
    pub const fn first_node(&self) -> u8 {
        self.nodes[0] % 40
    }
    #[inline(always)]
    pub fn child_nodes(&self) -> &[u8] {
        &self.nodes[1..]
    }
    #[inline(always)]
    pub const fn as_bytes(&self) -> &[u8] {
        &self.nodes
    }
}

impl Deref for ObjectIdentifier {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        &self.nodes
    }
}

impl FromStr for ObjectIdentifier {
    type Err = ObjectIdentifierError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut nodes = value.split(".");
        let root = nodes
            .next()
            .and_then(|n| n.parse::<u8>().ok())
            .and_then(|n| n.try_into().ok())
            .ok_or(ObjectIdentifierError::IllegalRootNode)?;

        let first = nodes
            .next()
            .and_then(|n| parse_string_first_node(n).ok())
            .ok_or(ObjectIdentifierError::IllegalFirstChildNode)?;

        let mut children = if let (_, Some(hint)) = nodes.size_hint() {
            Vec::with_capacity(hint)
        } else {
            Vec::new()
        };

        for child in nodes.map(|n| n.parse().ok()) {
            if let Some(c) = child {
                children.push(c);
            } else {
                return Err(ObjectIdentifierError::IllegalChildNodeValue);
            }
        }

        ObjectIdentifier::build(root, first, children)
    }
}

impl fmt::Display for ObjectIdentifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let show: String = self.into();
        write!(f, "{}", show)
    }
}
impl fmt::Debug for ObjectIdentifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let show: String = self.into();
        write!(f, "{}", show)
    }
}

#[repr(transparent)]
pub struct ArchivedObjectIdentifier {
    archived: ArchivedVec<u8>,
}

impl Deref for ArchivedObjectIdentifier {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        self.archived.as_slice()
    }
}

impl fmt::Debug for ArchivedObjectIdentifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            &convert_to_string(self.archived.as_slice())
                .unwrap_or_else(|e| format!("Invalid OID: {:?}", e))
        )
    }
}

impl Archive for ObjectIdentifier {
    type Archived = ArchivedObjectIdentifier;
    type Resolver = VecResolver;

    unsafe fn resolve(&self, pos: usize, resolver: Self::Resolver, out: *mut Self::Archived) {
        let (oid_pos, oid_out) = rkyv::out_field!(out.archived);
        ArchivedVec::resolve_from_slice(self.nodes.as_ref(), pos + oid_pos, resolver, oid_out);
    }
}
impl Archive for &'static ObjectIdentifier {
    type Archived = ArchivedObjectIdentifier;
    type Resolver = VecResolver;

    unsafe fn resolve(&self, pos: usize, resolver: Self::Resolver, out: *mut Self::Archived) {
        let (oid_pos, oid_out) = rkyv::out_field!(out.archived);
        ArchivedVec::resolve_from_slice(self.nodes.as_ref(), pos + oid_pos, resolver, oid_out);
    }
}

impl<S: Serializer + ?Sized> Serialize<S> for ObjectIdentifier
where
    [u8]: rkyv::SerializeUnsized<S>,
{
    fn serialize(&self, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
        ArchivedVec::serialize_from_slice(self.nodes.as_ref(), serializer)
    }
}

fn parse_string_first_node(first_child_node: &str) -> Result<u8, ObjectIdentifierError> {
    let first_child_node: u8 = first_child_node
        .parse()
        .map_err(|_| ObjectIdentifierError::IllegalFirstChildNode)?;
    if first_child_node > 39 {
        return Err(ObjectIdentifierError::IllegalFirstChildNode);
    }
    Ok(first_child_node)
}

impl ObjectIdentifier {
    fn from_string<S>(value: S) -> Result<ObjectIdentifier, ObjectIdentifierError>
    where
        S: AsRef<str>,
    {
        ObjectIdentifier::from_str(value.as_ref())
    }
}

fn convert_to_string(nodes: &[u8]) -> Result<String, ObjectIdentifierError> {
    assert!(nodes.len() > 0);
    let root = nodes[0] / 40;
    let mut out = root.to_string();

    out.push('.');
    let first = nodes[0] % 40;
    out.extend(first.to_string().chars());

    let mut parsing_big_int = false;
    let mut big_int: Node = 0;
    for i in 1..nodes.len() {
        if !parsing_big_int && nodes[i] < 128 {
            // less than 7 bit of node value
            out.push('.');
            let nr = nodes[i].to_string();
            out.extend(nr.chars());
        } else {
            if big_int > 0 {
                if big_int >= Node::MAX >> 7 {
                    return Err(ObjectIdentifierError::IllegalChildNodeValue);
                }
                big_int <<= 7;
            };
            big_int += (nodes[i] & !0x80) as Node;
            parsing_big_int = nodes[i] & 0x80 != 0;
        }
        if big_int > 0 && !parsing_big_int {
            out.push('.');
            out.extend(big_int.to_string().chars());
            big_int = 0;
        }
    }

    Ok(out)
}

impl Into<String> for &ObjectIdentifier {
    fn into(self) -> String {
        convert_to_string(&self.nodes).expect("Valid OID object couldn't be serialized.")
    }
}

impl Into<String> for ObjectIdentifier {
    fn into(self) -> String {
        (&self).into()
    }
}

impl<'a> Into<&'a [u8]> for &'a ObjectIdentifier {
    fn into(self) -> &'a [u8] {
        &self.nodes
    }
}

impl Into<Vec<u8>> for ObjectIdentifier {
    fn into(self) -> Vec<u8> {
        self.nodes.into_vec()
    }
}

impl TryFrom<&str> for ObjectIdentifier {
    type Error = ObjectIdentifierError;
    fn try_from(value: &str) -> Result<ObjectIdentifier, Self::Error> {
        ObjectIdentifier::from_string(value)
    }
}

impl TryFrom<String> for ObjectIdentifier {
    type Error = ObjectIdentifierError;
    fn try_from(value: String) -> Result<ObjectIdentifier, Self::Error> {
        ObjectIdentifier::from_string(value)
    }
}

impl TryFrom<&[u8]> for ObjectIdentifier {
    type Error = ObjectIdentifierError;
    fn try_from(nodes: &[u8]) -> Result<ObjectIdentifier, Self::Error> {
        Self::from_box(nodes.into())
    }
}

impl TryFrom<Vec<u8>> for ObjectIdentifier {
    type Error = ObjectIdentifierError;
    fn try_from(value: Vec<u8>) -> Result<ObjectIdentifier, Self::Error> {
        Self::from_box(value.into_boxed_slice())
    }
}

mod serde_support {
    use super::*;
    use core::fmt;
    use serde::{de, ser};

    struct OidVisitor;

    impl<'de> de::Visitor<'de> for OidVisitor {
        type Value = ObjectIdentifier;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a valid buffer representing an OID")
        }

        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            ObjectIdentifier::try_from(v).map_err(|err| {
                E::invalid_value(
                    de::Unexpected::Other(match err {
                        ObjectIdentifierError::IllegalRootNode => "illegal root node",
                        ObjectIdentifierError::IllegalFirstChildNode => "illegal first child node",
                        ObjectIdentifierError::IllegalChildNodeValue => "illegal child node value",
                    }),
                    &"a valid buffer representing an OID",
                )
            })
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            ObjectIdentifier::try_from(v).map_err(|err| {
                E::invalid_value(
                    de::Unexpected::Other(match err {
                        ObjectIdentifierError::IllegalRootNode => "illegal root node",
                        ObjectIdentifierError::IllegalFirstChildNode => "illegal first child node",
                        ObjectIdentifierError::IllegalChildNodeValue => "illegal child node value",
                    }),
                    &"a string representing an OID",
                )
            })
        }
    }

    impl<'de> de::Deserialize<'de> for ObjectIdentifier {
        fn deserialize<D>(deserializer: D) -> Result<ObjectIdentifier, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            if deserializer.is_human_readable() {
                deserializer.deserialize_str(OidVisitor)
            } else {
                deserializer.deserialize_bytes(OidVisitor)
            }
        }
    }

    impl ser::Serialize for ObjectIdentifier {
        fn serialize<S>(
            &self,
            serializer: S,
        ) -> Result<<S as ser::Serializer>::Ok, <S as ser::Serializer>::Error>
        where
            S: ser::Serializer,
        {
            if serializer.is_human_readable() {
                let encoded: String = self.into();
                serializer.serialize_str(&encoded)
            } else {
                serializer.serialize_bytes(self.as_bytes())
            }
        }
    }
    impl ser::Serialize for ArchivedObjectIdentifier {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: ser::Serializer,
        {
            if serializer.is_human_readable() {
                let encoded: String =
                    convert_to_string(self.deref()).expect("Failed to convert valid OID to String");
                serializer.serialize_str(&encoded)
            } else {
                serializer.serialize_bytes(self.deref())
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::convert::TryInto;

    pub(crate) fn gen_random() -> ObjectIdentifier {
        let amt: u8 = rand::random::<u8>() % 10 + 1;
        let mut children = Vec::new();
        for _ in 0..amt {
            children.push(rand::random());
        }

        ObjectIdentifier::build(ObjectIdentifierRoot::JointIsoItuT, 25, children).unwrap()
    }

    #[test]
    fn encode_binary_root_node_0() {
        let expected: Vec<u8> = vec![0];
        let oid = ObjectIdentifier::build(ObjectIdentifierRoot::ItuT, 0x00, vec![]).unwrap();
        let actual: Vec<u8> = oid.into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn encode_binary_root_node_1() {
        let expected: Vec<u8> = vec![40];
        let oid = ObjectIdentifier::build(ObjectIdentifierRoot::Iso, 0x00, vec![]).unwrap();
        let actual: Vec<u8> = oid.into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn encode_binary_root_node_2() {
        let expected: Vec<u8> = vec![80];
        let oid =
            ObjectIdentifier::build(ObjectIdentifierRoot::JointIsoItuT, 0x00, vec![]).unwrap();
        let actual: Vec<u8> = oid.into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn encode_binary_example_1() {
        let expected: Vec<u8> = vec![0x01, 0x01, 0x02, 0x03, 0x05, 0x08, 0x0D, 0x15];
        let oid = ObjectIdentifier::build(
            ObjectIdentifierRoot::ItuT,
            0x01,
            vec![1, 2, 3, 5, 8, 13, 21],
        )
        .unwrap();
        let actual: Vec<u8> = oid.into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn encode_binary_example_2() {
        let expected: Vec<u8> = vec![
            0x77, 0x2A, 0x93, 0x45, 0x83, 0xFF, 0x7F, 0x87, 0xFF, 0xFF, 0xFF, 0x7F, 0x89, 0x53,
            0x92, 0x30,
        ];
        let oid = ObjectIdentifier::build(
            ObjectIdentifierRoot::JointIsoItuT,
            39,
            vec![42, 2501, 65535, 2147483647, 1235, 2352],
        )
        .unwrap();
        let actual: Vec<u8> = (oid).into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn encode_string_root_node_0() {
        let expected = "0.0";
        let oid = ObjectIdentifier::build(ObjectIdentifierRoot::ItuT, 0x00, vec![]).unwrap();
        let actual: String = (oid).into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn encode_string_root_node_1() {
        let expected = "1.0";
        let oid = ObjectIdentifier::build(ObjectIdentifierRoot::Iso, 0x00, vec![]).unwrap();
        let actual: String = (&oid).into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn encode_string_root_node_2() {
        let expected = "2.0";
        let oid =
            ObjectIdentifier::build(ObjectIdentifierRoot::JointIsoItuT, 0x00, vec![]).unwrap();
        let actual: String = (&oid).into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn encode_string_example_1() {
        let expected = "0.1.1.2.3.5.8.13.21";
        let oid = ObjectIdentifier::build(
            ObjectIdentifierRoot::ItuT,
            0x01,
            vec![1, 2, 3, 5, 8, 13, 21],
        )
        .unwrap();
        let actual: String = (&oid).into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn encode_string_example_2() {
        let expected = "2.39.42.2501.65535.2147483647.1235.2352";
        let oid = ObjectIdentifier::build(
            ObjectIdentifierRoot::JointIsoItuT,
            39,
            vec![42, 2501, 65535, 2147483647, 1235, 2352],
        )
        .unwrap();
        let actual: String = (&oid).into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_binary_root_node_0() {
        let expected = ObjectIdentifier::build(ObjectIdentifierRoot::ItuT, 0x00, vec![]);
        let actual = vec![0x00].try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_binary_root_node_1() {
        let expected = ObjectIdentifier::build(ObjectIdentifierRoot::Iso, 0x00, vec![]);
        let actual = vec![40].try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_binary_root_node_2() {
        let expected = ObjectIdentifier::build(ObjectIdentifierRoot::JointIsoItuT, 0x00, vec![]);
        let actual = vec![80].try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_binary_example_1() {
        let expected = ObjectIdentifier::build(
            ObjectIdentifierRoot::ItuT,
            0x01,
            vec![1, 2, 3, 5, 8, 13, 21],
        );
        let actual = vec![0x01, 0x01, 0x02, 0x03, 0x05, 0x08, 0x0D, 0x15].try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_binary_example_2() {
        let expected = ObjectIdentifier::build(
            ObjectIdentifierRoot::JointIsoItuT,
            39,
            vec![42, 2501, 65535, 2147483647, 1235, 2352],
        );
        let actual = vec![
            0x77, 0x2A, 0x93, 0x45, 0x83, 0xFF, 0x7F, 0x87, 0xFF, 0xFF, 0xFF, 0x7F, 0x89, 0x53,
            0x92, 0x30,
        ]
        .try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_string_root_node_0() {
        let expected = ObjectIdentifier::build(ObjectIdentifierRoot::ItuT, 0x00, vec![]);
        let actual = "0.0".try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_string_root_node_1() {
        let expected = ObjectIdentifier::build(ObjectIdentifierRoot::Iso, 0x00, vec![]);
        let actual = "1.0".try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_string_root_node_2() {
        let expected = ObjectIdentifier::build(ObjectIdentifierRoot::JointIsoItuT, 0x00, vec![]);
        let actual = "2.0".try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_string_example_1() {
        let expected = ObjectIdentifier::build(
            ObjectIdentifierRoot::ItuT,
            0x01,
            vec![1, 2, 3, 5, 8, 13, 21],
        );
        let actual = "0.1.1.2.3.5.8.13.21".try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_string_example_2() {
        let expected = ObjectIdentifier::build(
            ObjectIdentifierRoot::JointIsoItuT,
            39,
            vec![42, 2501, 65535, 2147483647, 1235, 2352],
        );
        let actual = "2.39.42.2501.65535.2147483647.1235.2352".try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn illegal_oid_root() {
        let expected = Err(ObjectIdentifierError::IllegalRootNode);
        for i in 3..core::u8::MAX {
            let actual = ObjectIdentifierRoot::try_from(i);
            assert_eq!(expected, actual);
        }
    }

    #[test]
    fn illegal_first_node_too_large() {
        let expected = Err(ObjectIdentifierError::IllegalFirstChildNode);
        for i in 40..core::u8::MAX {
            let string_val = format!("{}.2.3.4", i);
            let mut nodes_iter = string_val.split(".");
            let actual = parse_string_first_node(nodes_iter.next().unwrap());
            assert_eq!(expected, actual);
        }
    }

    #[test]
    fn illegal_first_node_empty() {
        let expected = Err(ObjectIdentifierError::IllegalFirstChildNode);
        let string_val = String::new();
        let mut nodes_iter = string_val.split(".");
        let actual = parse_string_first_node(nodes_iter.next().unwrap());
        assert_eq!(expected, actual);
    }

    #[test]
    fn illegal_first_node_large() {
        let expected = Err(ObjectIdentifierError::IllegalFirstChildNode);
        let string_val = String::from("40");
        let mut nodes_iter = string_val.split(".");
        let actual = parse_string_first_node(nodes_iter.next().unwrap());
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_string_crap() {
        let expected: Result<ObjectIdentifier, ObjectIdentifierError> =
            Err(ObjectIdentifierError::IllegalRootNode);
        let actual = "wtf".try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_string_empty() {
        let expected: Result<ObjectIdentifier, ObjectIdentifierError> =
            Err(ObjectIdentifierError::IllegalRootNode);
        let actual = String::new().try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_binary_empty() {
        let expected: Result<ObjectIdentifier, ObjectIdentifierError> =
            Err(ObjectIdentifierError::IllegalRootNode);
        let actual = vec![].try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_binary_example_over_u128() {
        let expected: Result<ObjectIdentifier, ObjectIdentifierError> =
            Err(ObjectIdentifierError::IllegalChildNodeValue);
        let actual = vec![
            0x00, 0x89, 0x97, 0xBF, 0xA3, 0xB8, 0xE8, 0xB3, 0xE6, 0xFB, 0xF2, 0xEA, 0xC3, 0xCA,
            0xF2, 0xBF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F,
        ]
        .try_into();
        assert_eq!(expected, actual);
    }
    #[test]
    fn parse_string_root_node_3plus() {
        for i in 3..=core::u8::MAX {
            let expected: Result<ObjectIdentifier, ObjectIdentifierError> =
                Err(ObjectIdentifierError::IllegalRootNode);
            let actual = format!("{}", i).try_into();
            assert_eq!(expected, actual);
        }
    }

    #[test]
    fn parse_string_example_over_u128() {
        let expected: Result<ObjectIdentifier, ObjectIdentifierError> =
            Err(ObjectIdentifierError::IllegalChildNodeValue);
        let actual = "1.1.349239782398732987223423423423423423423423423423434982342342342342342342324523453452345234523452345234523452345234537234987234".try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_string_example_first_node_over_39() {
        let expected: Result<ObjectIdentifier, ObjectIdentifierError> =
            Err(ObjectIdentifierError::IllegalFirstChildNode);
        let actual = "1.40.1.2.3".try_into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn parse_string_large_children_ok() {
        let expected = ObjectIdentifier::build(
            ObjectIdentifierRoot::JointIsoItuT,
            25,
            vec![
                190754093376743485973207716749546715206,
                255822649272987943607843257596365752308,
                15843412533224453995377625663329542022,
                6457999595881951503805148772927347934,
                19545192863105095042881850060069531734,
                195548685662657784196186957311035194990,
                233020488258340943072303499291936117654,
                193307160423854019916786016773068715190,
            ],
        )
        .unwrap();
        let actual = "2.25.190754093376743485973207716749546715206.\
                           255822649272987943607843257596365752308.\
                           15843412533224453995377625663329542022.\
                           6457999595881951503805148772927347934.\
                           19545192863105095042881850060069531734.\
                           195548685662657784196186957311035194990.\
                           233020488258340943072303499291936117654.\
                           193307160423854019916786016773068715190"
            .try_into()
            .unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn encode_to_string() {
        let expected = String::from("1.2.3.4");
        let actual: String = ObjectIdentifier::build(ObjectIdentifierRoot::Iso, 2, vec![3, 4])
            .unwrap()
            .into();
        assert_eq!(expected, actual);
    }

    #[test]
    fn encode_to_bytes() {
        let expected = vec![0x2A, 0x03, 0x04];
        let actual: Vec<u8> = ObjectIdentifier::build(ObjectIdentifierRoot::Iso, 2, vec![3, 4])
            .unwrap()
            .into();
        assert_eq!(expected, actual);
    }
}
