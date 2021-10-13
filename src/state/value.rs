use core::{
    ptr,
};
use std::{
    fmt,
    any::Any,
    hash::Hash,
    ops::Deref,
    convert::TryFrom,
};

use rkyv::{Archive, Archived, Serialize, Deserialize, out_field, };
use rkyv_dyn::{archive_dyn, DynSerializer, DynError, DynDeserializer};
use rkyv_typename::TypeName;
use ptr_meta::{DynMetadata, Pointee};
use std::marker::PhantomData;

use inventory;
use crate::oid::{ObjectIdentifier};
use rkyv::ser::{Serializer, };
use std::collections::HashMap;
use std::alloc::Layout;
use serde::ser::SerializeMap;
use std::fmt::Formatter;
use serde::de::Error as _;


pub trait Value: fmt::Debug + erased_serde::Serialize {
    fn deserialize_val_in_place<'de>(&mut self, deserializer: &mut dyn erased_serde::Deserializer<'de>)
        -> Result<(), erased_serde::Error>;
}
erased_serde::serialize_trait_object!(Value);

impl<T> Value for T
    where T: fmt::Debug + Archive + erased_serde::Serialize + for<'de> serde::Deserialize<'de>
{
    fn deserialize_val_in_place<'de>(&mut self, deserializer: &mut dyn erased_serde::Deserializer<'de>)
        -> Result<(), erased_serde::Error>
    {
        *self = erased_serde::deserialize(deserializer)?;
        Ok(())
    }
}

impl Pointee for dyn Value {
    type Metadata = DynMetadata<dyn Value>;
}

#[derive(Debug)]
pub struct Entry<'a> {
    pub oid: &'a ObjectIdentifier,
    pub val: &'a dyn Value,
}
#[derive(Debug)]
pub struct OwnedEntry {
    pub oid: ObjectIdentifier,
    pub val: Box<dyn Value>,
}

impl<'a> serde::Serialize for Entry<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: serde::Serializer
    {
        let mut ser = serializer.serialize_map(Some(1))?;
        ser.serialize_entry(self.oid, self.val)?;
        ser.end()
    }
}

impl<'de> serde::Deserialize<'de> for OwnedEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: serde::Deserializer<'de>
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

    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error>
    {
        // Bad magic code. Problem we have to solve: We only know how to parse whatever comes
        // after the OID after having looked at the OID. We have zero static type info available
        // during deserialization. Soooooo:

        // Get OID first. That's easy, we know it's the key, we know how to read it.
        let oid: ObjectIdentifier = map.next_key()?
            .ok_or(A::Error::missing_field("oid"))?;
        let b: Vec<u8> = oid.clone().into();

        // Get the Value vtable for that OID. Or fail because we don't know that OID, either works.
        let valimpl = IMPL_REGISTRY.get(ImplId::from_type_oid(&b))
            .ok_or(serde::de::Error::invalid_value(
                serde::de::Unexpected::Other("unknown oid"),
                &"oid an implementation was registered for"
            ))?;

        // Casting random usize you find on the side of the road as vtable on unchecked pointers.
        // What could possibly go wrong? >:D
        let valbox: Box<dyn Value> = unsafe {
            // recreate vtable as fat ptr metadata
            let meta = valimpl.pointer_metadata();

            // Don't bother checking here. The only way this could be bad is if the vtable above
            // is bad an in that case a segfault here would be *much better* than whatever is
            // going to happen afterwards.
            let layout = Layout::from_size_align_unchecked(meta.size_of(), meta.align_of());

            // Hello yes I would like a Box the old fashioned way.
            // Oh you're asking why we're allocating stuff here and never ever bother zeroing or
            // validate in any other way if this is sane?
            // Well...
            let ptr: *mut () = std::alloc::alloc(layout).cast::<()>();
            Box::from_raw(ptr_meta::from_raw_parts_mut(ptr, meta))
        };
        // ... The only way we can make Value a trait object by having it deserialize *into
        // it's own uninitialized representation*. Yeah don't worry, this isn't the worst part of
        // the game yet. >:D
        let seed = ValueSeed(valbox);
        let val = map.next_value_seed(seed)?;
        Ok(OwnedEntry { oid, val })
    }
}
struct ValueSeed(Box<dyn Value>);
impl<'de> serde::de::DeserializeSeed<'de> for ValueSeed {
    type Value = Box<dyn Value>;

    fn deserialize<D>(mut self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: serde::Deserializer<'de>
    {
        let mut deser = <dyn erased_serde::Deserializer>::erase(deserializer);
        // Hey, better initialize late than never. Oh completely unrelated but if we unwind after
        // allocating the box and before completing this function call that's undefined behaviour
        // so maybe don't do that thanks <3
        self.0.deserialize_val_in_place(&mut deser);
        Ok(self.0)
    }
}

pub trait TypeOid {
    fn get_type_oid() -> ObjectIdentifier;
}

impl TypeOid for Archived<bool> {
    fn get_type_oid() -> ObjectIdentifier {
        ObjectIdentifier::try_from("1.3.6.1.4.1.48398.612.1.1").unwrap()
    }
}

pub trait SerializeValue {
    fn serialize_val(&self, serializer: &mut dyn DynSerializer) -> Result<usize, DynError>;
    fn archived_type_oid(&self) -> ObjectIdentifier;
}

impl<T> SerializeValue for T
    where T: for<'a> Serialize<dyn DynSerializer + 'a>,
          T::Archived: TypeOid,
{
    fn serialize_val(&self, serializer: &mut dyn DynSerializer) -> Result<usize, DynError> {
        serializer.serialize_value(self)
    }

    fn archived_type_oid(&self) -> ObjectIdentifier {
        Archived::<T>::get_type_oid()
    }
}

trait DeserializeValue<T: Pointee + ?Sized> {
    unsafe fn deserialize_val(
        &self,
        deserializer: &mut dyn DynDeserializer,
        alloc: &mut dyn FnMut(Layout) -> *mut u8,
    ) -> Result<*mut (), DynError>;

    fn deserialize_dyn_metadata(
        &self,
        deserializer: &mut dyn DynDeserializer,
    ) -> Result<T::Metadata, DynError>;
}

pub struct ArchivedValueMetadata<T: ?Sized> {
    type_oid: Archived<ObjectIdentifier>,
    phantom: PhantomData<T>,
}

impl<T: TypeOid + ?Sized> ArchivedValueMetadata<T> {
    pub unsafe fn emplace(type_oid: Archived<ObjectIdentifier>, out: *mut Self) {
        ptr::addr_of_mut!((*out).type_oid).write(type_oid);
    }

    pub fn vtable(&self) -> usize {
        IMPL_REGISTRY
            .get(ImplId::from_type_oid(&self.type_oid))
            .expect("Unregistered type oid")
            .vtable
    }

    pub fn pointer_metadata(&self) -> DynMetadata<T> {
        unsafe { core::mem::transmute(self.vtable()) }
    }
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
struct ImplId<'a> {
    type_oid: &'a [u8],
}

impl<'a> ImplId<'a> {
    fn from_type_oid(type_oid: &'a [u8]) -> Self {
        Self { type_oid }
    }
}
impl ImplId<'static> {
    fn new<T: TypeOid>() -> Self {
        let oid: Vec<u8> = T::get_type_oid().into();
        Self {
            type_oid: oid.leak()
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct ImplData {
    pub vtable: usize,
    // TODO DebugImpl
    // TODO DebugInfo
}
impl ImplData {
    pub unsafe fn pointer_metadata<T: ?Sized>(&self) -> DynMetadata<T> {
        core::mem::transmute(self.vtable)
    }
}

struct ImplEntry<'a> {
    id: ImplId<'a>,
    data: ImplData,
}
inventory::collect!(ImplEntry<'static>);

impl ImplEntry<'_> {
    #[doc(hidden)]
    pub fn new<T: TypeOid + RegisteredImpl>() -> Self {
        Self {
            id: ImplId::new::<T>(),
            data: ImplData {
                vtable: <T as RegisteredImpl>::vtable(),
            },
        }
    }
}

struct ImplRegistry {
    oid_to_data: HashMap<ImplId<'static>, ImplData>,
}

impl ImplRegistry {
    fn new() -> Self {
        Self { oid_to_data: HashMap::new() }
    }

    fn add_entry(&mut self, entry: &'static ImplEntry) {
        let old_val = self.oid_to_data.insert(entry.id, entry.data);
        assert!(old_val.is_none());
    }

    fn get(&self, type_oid: ImplId) -> Option<ImplData> {
        self.oid_to_data.get(&type_oid).map(|d| *d)
    }
}

lazy_static::lazy_static! {
    // FIXME: Dynamic modules *will* break this.
    static ref IMPL_REGISTRY: ImplRegistry = {
        let mut reg = ImplRegistry::new();
        for entry in inventory::iter::<ImplEntry> {
            reg.add_entry(entry);
        }
        reg
    };
}

pub unsafe trait RegisteredImpl {
    fn vtable() -> usize;
}

unsafe impl RegisteredImpl for bool {
    fn vtable() -> usize {
        unsafe {
            core::mem::transmute(ptr_meta::metadata(
                core::ptr::null::<bool>() as *const dyn Value
            ))
        }
    }
}
inventory::submit! {ImplEntry::new::<bool>()}

#[archive_dyn(deserialize)]
/// Trait to be implemented by any value in the state map.
///
/// A value can be any type not having dangling references (with the added
/// restriction that it has to implement `Debug` for debugger QoL).
/// In fact Value *also* needs to implement Hash since BFFH checks if the state
/// is different to before on input and output before updating the resource re.
/// notifying actors and notifys.  This dependency is not expressable via
/// supertraits since it is not possible to make Hash into a trait object.
/// To solve this [`State`] uses the [`StateBuilder`] which adds an `Hash`
/// requirement for inputs on [`add`](struct::StateBuilder::add). The hash is
/// being created over all inserted values and then used to check for equality.
/// Note that in addition to collisions, Hash is not guaranteed stable over
/// ordering and will additionally not track overwrites, so if the order of
/// insertions changes or values are set and later overwritten then two equal
/// States can and are likely to have different hashes.
pub trait DynValue: Any + fmt::Debug {}

macro_rules! valtype {
    ( $x:ident, $y:ident ) => {
        #[repr(transparent)]
        #[derive(Debug, PartialEq, Eq, Hash)]
        #[derive(Archive, Serialize, Deserialize)]
        #[derive(serde::Serialize, serde::Deserialize)]
        #[archive_attr(derive(TypeName, Debug))]
        pub struct $x(pub $y);

        #[archive_dyn(deserialize)]
        impl DynValue for $x {}
        impl DynValue for Archived<$x> {}

        impl Deref for $x {
            type Target = $y;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl From<$y> for $x {
            fn from(e: $y) -> $x {
                Self(e)
            }
        }

        impl $x {
            pub fn new(e: $y) -> Self {
                Self(e)
            }

            pub fn into_inner(self) -> $y {
                self.0
            }
        }
    }
}

valtype!(Bool, bool);
valtype!(UInt8, u8);
valtype!(UInt16, u16);
valtype!(UInt32, u32);
valtype!(UInt64, u64);
valtype!(UInt128, u128);
valtype!(Int8, i8);
valtype!(Int16, i16);
valtype!(Int32, i32);
valtype!(Int64, i64);
valtype!(Int128, i128);
valtype!(RString, String);

#[derive(serde::Serialize, serde::Deserialize)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Archive, Serialize, Deserialize)]
#[archive_attr(derive(TypeName, Debug))]
pub struct Vec3u8 {
    pub a: u8,
    pub b: u8,
    pub c: u8,
}

#[archive_dyn(deserialize)]
impl DynValue for Vec3u8 {}

impl DynValue for Archived<Vec3u8> {}

#[cfg(test)]
mod tests {
}