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

use rkyv::{Archive, Archived, Serialize, Deserialize, out_field, Fallible, DeserializeUnsized, ArchivePointee, ArchiveUnsized, ArchivedMetadata, SerializeUnsized};
use rkyv_dyn::{archive_dyn, DynSerializer, DynError, DynDeserializer};
use rkyv_typename::TypeName;
use ptr_meta::{DynMetadata, Pointee};

use inventory;
use crate::oid::{ObjectIdentifier};
use rkyv::ser::{Serializer, ScratchSpace};
use std::collections::HashMap;
use std::alloc::Layout;
use serde::ser::SerializeMap;
use std::fmt::Formatter;
use serde::de::Error as _;
use std::mem::MaybeUninit;


pub trait Value: fmt::Debug + erased_serde::Serialize {
    /// Initialize `&mut self` from `deserializer`
    ///
    /// At the point this is called &mut self is of undefined value but guaranteed to be well
    /// aligned and non-null. Any read access into &mut self before all of &self is brought into
    /// a valid state is however undefined behaviour.
    /// To this end you *must* initialize `self` **completely**. Serde will do the right thing if
    /// you directly deserialize the type you're implementing `Value` for, but for manual
    /// implementations this is important to keep in mind.
    fn deserialize_init<'de>(&mut self, deserializer: &mut dyn erased_serde::Deserializer<'de>)
                                     -> Result<(), erased_serde::Error>;
}
erased_serde::serialize_trait_object!(Value);

impl<T> Value for T
    where T: fmt::Debug
           + erased_serde::Serialize
           + for<'de> serde::Deserialize<'de>
{
    fn deserialize_init<'de>(&mut self, deserializer: &mut dyn erased_serde::Deserializer<'de>)
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
    pub val: Box<dyn DeserializeValue>,
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
                &"oid an implementation was registered for",
            ))?;

        // Casting random usize you find on the side of the road as vtable on unchecked pointers.
        // What could possibly go wrong? >:D
        let valbox: MaybeUninit<Box<dyn DeserializeValue>> = unsafe {
            // "recreate" vtable as fat ptr metadata (we literally just cast an `usize` but the
            // only way to put this usize into that spot is by having a valid vtable cast so it's
            // probably almost safe)
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
            let b = Box::from_raw(ptr_meta::from_raw_parts_mut(
                ptr,
                meta));

            // We make this a MaybeUninit so `Drop` is never called on the uninitialized value
            MaybeUninit::new(b)
        };
        // ... The only way we can make Value a trait object by having it deserialize *into
        // it's own uninitialized representation*. Yeah don't worry, this isn't the worst part of
        // the game yet. >:D
        let seed = InitIntoSelf(valbox);
        let val = map.next_value_seed(seed)?;
        Ok(OwnedEntry { oid, val })
    }
}

struct InitIntoSelf(MaybeUninit<Box<dyn DeserializeValue>>);

impl<'de> serde::de::DeserializeSeed<'de> for InitIntoSelf {
    type Value = Box<dyn DeserializeValue>;

    fn deserialize<D>(mut self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: serde::Deserializer<'de>
    {
        let mut deser = <dyn erased_serde::Deserializer>::erase(deserializer);

        // Unsafe as hell but if we never read from this reference before initializing it's not
        // undefined behaviour.
        let selfptr = unsafe { &mut *self.0.as_mut_ptr() };

        // Hey, better initialize late than never.
        selfptr.deserialize_init(&mut deser).map_err(|e|
            D::Error::custom(e))?;

        // Assuming `deserialize_init` didn't error and did its job this is now safe.
        unsafe { Ok(self.0.assume_init()) }
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
impl DeserializeDynOid for Archived<bool>
    where Archived<bool>: for<'a> Deserialize<bool, (dyn DynDeserializer + 'a)>
{
    unsafe fn deserialize_dynoid(&self, deserializer: &mut dyn DynDeserializer, alloc: &mut dyn FnMut(Layout) -> *mut u8) -> Result<*mut (), DynError> {
        let ptr = alloc(Layout::new::<bool>()).cast::<bool>();
        ptr.write(self.deserialize(deserializer)?);
        Ok(ptr as *mut ())
    }

    fn deserialize_dynoid_metadata(&self, deserializer: &mut dyn DynDeserializer) -> Result<<dyn SerializeValue as Pointee>::Metadata, DynError> {
        unsafe {
            Ok(core::mem::transmute(ptr_meta::metadata(
                core::ptr::null::<bool>() as *const dyn SerializeValue
            )))
        }
    }
}
impl<S: ScratchSpace + Serializer + ?Sized> SerializeUnsized<S> for dyn SerializeValue {
    fn serialize_unsized(&self, mut serializer: &mut S) -> Result<usize, S::Error> {
        self.serialize_dynoid(&mut serializer)
            .map_err(|e| *e.downcast::<S::Error>().unwrap())
    }

    fn serialize_metadata(&self, mut serializer: &mut S) -> Result<Self::MetadataResolver, S::Error> {
        self.serialize_metadata(serializer)
    }
}


/// Serialize dynamic types by storing an OID alongside
pub trait SerializeDynOid {
    fn serialize_dynoid(&self, serializer: &mut dyn DynSerializer) -> Result<usize, DynError>;
    fn archived_type_oid(&self) -> ObjectIdentifier;
}

impl<T> SerializeDynOid for T
    where T: for<'a> Serialize<dyn DynSerializer + 'a>,
          T::Archived: TypeOid,
{
    fn serialize_dynoid(&self, serializer: &mut dyn DynSerializer) -> Result<usize, DynError> {
        serializer.serialize_value(self)
    }

    fn archived_type_oid(&self) -> ObjectIdentifier {
        Archived::<T>::get_type_oid()
    }
}

trait DeserializeDynOid {
    unsafe fn deserialize_dynoid(
        &self,
        deserializer: &mut dyn DynDeserializer,
        alloc: &mut dyn FnMut(Layout) -> *mut u8,
    ) -> Result<*mut (), DynError>;

    fn deserialize_dynoid_metadata(
        &self,
        deserializer: &mut dyn DynDeserializer,
    ) -> Result<<dyn SerializeValue as Pointee>::Metadata, DynError>;
}

#[ptr_meta::pointee]
pub trait SerializeValue: Value + SerializeDynOid {}

impl<T: Archive + SerializeDynOid + Value> SerializeValue for T
    where
        T::Archived: RegisteredImpl
{}

#[ptr_meta::pointee]
pub trait DeserializeValue: Value + DeserializeDynOid {}
impl<T: Value + DeserializeDynOid> DeserializeValue for T {}
impl ArchivePointee for dyn DeserializeValue {
    type ArchivedMetadata = ArchivedValueMetadata;

    fn pointer_metadata(archived: &Self::ArchivedMetadata) -> <Self as Pointee>::Metadata {
        archived.pointer_metadata()
    }
}
impl<D: ?Sized> DeserializeUnsized<dyn SerializeValue, D> for dyn DeserializeValue
    where D: Fallible + DynDeserializer
{
    unsafe fn deserialize_unsized(&self,
                                  mut deserializer: &mut D,
                                  mut alloc: impl FnMut(Layout) -> *mut u8
    ) -> Result<*mut (), D::Error> {
        self.deserialize_dynoid(&mut deserializer, &mut alloc).map_err(|e| *e.downcast().unwrap())
    }

    fn deserialize_metadata(&self, mut deserializer: &mut D)
        -> Result<<dyn SerializeValue as Pointee>::Metadata, D::Error>
    {
        self.deserialize_dynoid_metadata(&mut deserializer).map_err(|e| *e.downcast().unwrap())
    }
}

impl ArchiveUnsized for dyn SerializeValue {
    type Archived = dyn DeserializeValue;
    type MetadataResolver = <ObjectIdentifier as Archive>::Resolver;

    unsafe fn resolve_metadata(&self, pos: usize, resolver: Self::MetadataResolver, out: *mut ArchivedMetadata<Self>) {
        let (oid_pos, oid) = out_field!(out.type_oid);
        let type_oid = self.archived_type_oid();
        type_oid.resolve(oid_pos, resolver, oid);
    }
}

pub struct ArchivedValueMetadata {
    type_oid: Archived<ObjectIdentifier>,
}

impl ArchivedValueMetadata {
    pub unsafe fn emplace(type_oid: Archived<ObjectIdentifier>, out: *mut Self) {
        ptr::addr_of_mut!((*out).type_oid).write(type_oid);
    }

    pub fn vtable(&self) -> usize {
        IMPL_REGISTRY
            .get(ImplId::from_type_oid(&self.type_oid)).expect("Unregistered type oid")
            .vtable
    }

    pub fn pointer_metadata(&self) -> DynMetadata<dyn DeserializeValue> {
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
                core::ptr::null::<bool>() as *const dyn DeserializeValue
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
mod tests {}