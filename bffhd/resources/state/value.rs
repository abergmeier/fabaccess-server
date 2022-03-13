use std::{any::Any, fmt, hash::Hash, ptr, str::FromStr};

use ptr_meta::{DynMetadata, Pointee};
use rkyv::{
    out_field, Archive, ArchivePointee, ArchiveUnsized, Archived, ArchivedMetadata, Deserialize,
    DeserializeUnsized, Fallible, Serialize, SerializeUnsized,
};
use rkyv_dyn::{DynDeserializer, DynError, DynSerializer};
use rkyv_typename::TypeName;

use crate::utils::oid::ObjectIdentifier;
use inventory;
use rkyv::ser::{ScratchSpace, Serializer};
use serde::de::Error as SerdeError;
use serde::ser::SerializeMap;
use std::alloc::Layout;
use std::collections::HashMap;
use std::fmt::Formatter;
use std::mem::MaybeUninit;

/// Adding a custom type to BFFH state management:
///
/// 1. Implement `serde`'s [`Serialize`](serde::Serialize) and [`Deserialize`](serde::Deserialize)
///     - `derive()`d instances work just fine, but keep stability over releases in mind.
/// 2. Implement rkyv's [`Serialize`](rkyv::Serialize).
/// 3. Implement TypeOid on your Archived type (i.e. `<T as Archive>::Archived`)
/// 4. Implement this
pub trait Value: Any + fmt::Debug + erased_serde::Serialize + Sync {
    /// Initialize `&mut self` from `deserializer`
    ///
    /// At the point this is called &mut self is of undefined value but guaranteed to be well
    /// aligned and non-null. Any read access into &mut self before all of &self is brought into
    /// a valid state is however undefined behaviour.
    /// To this end you *must* initialize `self` **completely**. Serde will do the right thing if
    /// you directly deserialize the type you're implementing `Value` for, but for manual
    /// implementations this is important to keep in mind.
    fn deserialize_init<'de>(
        &mut self,
        deserializer: &mut dyn erased_serde::Deserializer<'de>,
    ) -> Result<(), erased_serde::Error>;

    /// Implement `PartialEq` dynamically.
    ///
    /// This should return `true` iff the Value is of the same type and `self` == `other` for
    /// non-dynamic types would return `true`.
    /// It is safe to always return `false`.
    fn dyn_eq(&self, other: &dyn Value) -> bool;

    fn as_value(&self) -> &dyn Value;
    fn as_any(&self) -> &dyn Any;
}
erased_serde::serialize_trait_object!(Value);
erased_serde::serialize_trait_object!(SerializeValue);
erased_serde::serialize_trait_object!(DeserializeValue);

impl<T> Value for T
where
    T: Any
        + fmt::Debug
        + PartialEq
        + Sync
        + erased_serde::Serialize
        + for<'de> serde::Deserialize<'de>,
{
    fn deserialize_init<'de>(
        &mut self,
        deserializer: &mut dyn erased_serde::Deserializer<'de>,
    ) -> Result<(), erased_serde::Error> {
        *self = erased_serde::deserialize(deserializer)?;
        Ok(())
    }

    fn dyn_eq(&self, other: &dyn Value) -> bool {
        other
            .as_any()
            .downcast_ref()
            .map_or(false, |other: &T| other == self)
    }

    fn as_value(&self) -> &dyn Value {
        self
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl PartialEq for dyn Value {
    fn eq(&self, other: &Self) -> bool {
        self.dyn_eq(other)
    }
}

#[repr(transparent)]
pub(super) struct DynVal<'a>(pub &'a dyn SerializeValue);
impl<'a> serde::Serialize for DynVal<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut ser = serializer.serialize_map(Some(1))?;
        let oid = self.0.archived_type_oid();
        ser.serialize_entry(oid, self.0)?;
        ser.end()
    }
}
#[repr(transparent)]
pub(super) struct DynOwnedVal(pub Box<dyn SerializeValue>);
impl<'de> serde::Deserialize<'de> for DynOwnedVal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(DynValVisitor)
    }
}

struct DynValVisitor;

impl<'de> serde::de::Visitor<'de> for DynValVisitor {
    type Value = DynOwnedVal;

    fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "an one entry map from OID to some value object")
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        // Bad magic code. Problem we have to solve: We only know how to parse whatever comes
        // after the OID after having looked at the OID. We have zero static type info available
        // during deserialization. So:

        // Get OID first. That's easy, we know it's the key, we know how to read it.
        let oid: ObjectIdentifier = map.next_key()?.ok_or(A::Error::missing_field("oid"))?;

        // Get the Value vtable for that OID. Or fail because we don't know that OID, either works.
        let valimpl = IMPL_REGISTRY.get(ImplId::from_type_oid(&oid)).ok_or(
            serde::de::Error::invalid_value(
                serde::de::Unexpected::Other("unknown oid"),
                &"oid an implementation was registered for",
            ),
        )?;

        // Casting random usize you find on the side of the road as vtable on unchecked pointers.
        // What could possibly go wrong? >:D
        let valbox: MaybeUninit<Box<dyn SerializeValue>> = unsafe {
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
            let b = Box::from_raw(ptr_meta::from_raw_parts_mut(ptr, meta));

            // We make this a MaybeUninit so `Drop` is never called on the uninitialized value
            MaybeUninit::new(b)
        };
        // ... The only way we can make Value a trait object by having it deserialize *into
        // it's own uninitialized representation*. Yeah don't worry, this isn't the worst part of
        // the game yet. >:D
        let seed = InitIntoSelf(valbox);
        let val = map.next_value_seed(seed)?;
        Ok(DynOwnedVal(val))
    }
}

struct InitIntoSelf(MaybeUninit<Box<dyn SerializeValue>>);

impl<'de> serde::de::DeserializeSeed<'de> for InitIntoSelf {
    type Value = Box<dyn SerializeValue>;

    fn deserialize<D>(mut self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let mut deser = <dyn erased_serde::Deserializer>::erase(deserializer);

        // Unsafe as hell but if we never read from this reference before initializing it's not
        // undefined behaviour.
        let selfptr = unsafe { &mut *self.0.as_mut_ptr() };

        // Hey, better initialize late than never.
        selfptr
            .deserialize_init(&mut deser)
            .map_err(|e| D::Error::custom(e))?;

        // Assuming `deserialize_init` didn't error and did its job this is now safe.
        unsafe { Ok(self.0.assume_init()) }
    }
}

pub trait TypeOid {
    fn type_oid() -> &'static ObjectIdentifier;
    fn type_name() -> &'static str;
    fn type_desc() -> &'static str;
}

impl<S: ScratchSpace + Serializer + ?Sized> SerializeUnsized<S> for dyn SerializeValue {
    fn serialize_unsized(&self, mut serializer: &mut S) -> Result<usize, S::Error> {
        self.serialize_dynoid(&mut serializer)
            .map_err(|e| *e.downcast::<S::Error>().unwrap())
    }

    fn serialize_metadata(&self, serializer: &mut S) -> Result<Self::MetadataResolver, S::Error> {
        let oid = self.archived_type_oid();
        oid.serialize(serializer)
    }
}

/// Serialize dynamic types by storing an OID alongside
pub trait SerializeDynOid {
    fn serialize_dynoid(&self, serializer: &mut dyn DynSerializer) -> Result<usize, DynError>;
    fn archived_type_oid(&self) -> &'static ObjectIdentifier;
}

impl<T> SerializeDynOid for T
where
    T: for<'a> Serialize<dyn DynSerializer + 'a>,
    T::Archived: TypeOid,
{
    fn serialize_dynoid(&self, serializer: &mut dyn DynSerializer) -> Result<usize, DynError> {
        serializer.serialize_value(self)
    }

    fn archived_type_oid(&self) -> &'static ObjectIdentifier {
        Archived::<T>::type_oid()
    }
}

pub trait DeserializeDynOid {
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
pub trait SerializeValue: Value + SerializeDynOid {
    fn dyn_clone(&self) -> Box<dyn SerializeValue>;
}

impl<T: Archive + Value + SerializeDynOid + Clone> SerializeValue for T
where
    T::Archived: RegisteredImpl,
{
    fn dyn_clone(&self) -> Box<dyn SerializeValue> {
        Box::new(self.clone())
    }
}

impl PartialEq for dyn SerializeValue {
    fn eq(&self, other: &Self) -> bool {
        self.dyn_eq(other.as_value())
    }
}

impl Clone for Box<dyn SerializeValue> {
    fn clone(&self) -> Self {
        self.dyn_clone()
    }
}

#[ptr_meta::pointee]
pub trait DeserializeValue: DeserializeDynOid {}
impl<T: DeserializeDynOid> DeserializeValue for T {}
impl ArchivePointee for dyn DeserializeValue {
    type ArchivedMetadata = ArchivedValueMetadata;

    fn pointer_metadata(archived: &Self::ArchivedMetadata) -> <Self as Pointee>::Metadata {
        archived.pointer_metadata()
    }
}
impl<D: Fallible + ?Sized> DeserializeUnsized<dyn SerializeValue, D> for dyn DeserializeValue {
    unsafe fn deserialize_unsized(
        &self,
        mut deserializer: &mut D,
        mut alloc: impl FnMut(Layout) -> *mut u8,
    ) -> Result<*mut (), D::Error> {
        self.deserialize_dynoid(&mut deserializer, &mut alloc)
            .map_err(|e| *e.downcast().unwrap())
    }

    fn deserialize_metadata(
        &self,
        mut deserializer: &mut D,
    ) -> Result<<dyn SerializeValue as Pointee>::Metadata, D::Error> {
        self.deserialize_dynoid_metadata(&mut deserializer)
            .map_err(|e| *e.downcast().unwrap())
    }
}

impl ArchiveUnsized for dyn SerializeValue {
    type Archived = dyn DeserializeValue;
    type MetadataResolver = <ObjectIdentifier as Archive>::Resolver;

    unsafe fn resolve_metadata(
        &self,
        pos: usize,
        resolver: Self::MetadataResolver,
        out: *mut ArchivedMetadata<Self>,
    ) {
        let (oid_pos, oid) = out_field!(out.type_oid);
        let type_oid = self.archived_type_oid();
        type_oid.resolve(pos + oid_pos, resolver, oid);
    }
}

#[derive(Debug)]
pub struct ArchivedValueMetadata {
    pub type_oid: Archived<ObjectIdentifier>,
}

impl ArchivedValueMetadata {
    pub unsafe fn emplace(type_oid: Archived<ObjectIdentifier>, out: *mut Self) {
        let p = ptr::addr_of_mut!((*out).type_oid);
        ptr::write(p, type_oid);
    }

    pub fn vtable(&self) -> usize {
        IMPL_REGISTRY
            .get(ImplId::from_type_oid(&self.type_oid))
            .expect(&format!(
                "Unregistered \
            type \
            oid \
            {:?}",
                self.type_oid
            ))
            .vtable
    }

    pub fn pointer_metadata(&self) -> DynMetadata<dyn DeserializeValue> {
        unsafe { core::mem::transmute(self.vtable()) }
    }
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct ImplId<'a> {
    type_oid: &'a [u8],
}

impl<'a> ImplId<'a> {
    pub fn from_type_oid(type_oid: &'a [u8]) -> Self {
        Self { type_oid }
    }
}

impl ImplId<'static> {
    fn new<T: TypeOid>() -> Self {
        Self {
            type_oid: &T::type_oid(),
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct ImplData<'a> {
    pub vtable: usize,
    pub name: &'a str,
    pub desc: &'a str,
    pub info: ImplDebugInfo,
}

#[derive(Copy, Clone, Debug)]
#[doc(hidden)]
pub struct ImplDebugInfo {
    pub file: &'static str,
    pub line: u32,
    pub column: u32,
}

impl ImplData<'_> {
    pub unsafe fn pointer_metadata<T: ?Sized>(&self) -> DynMetadata<T> {
        core::mem::transmute(self.vtable)
    }
}

#[derive(Debug)]
pub struct ImplEntry<'a> {
    id: ImplId<'a>,
    data: ImplData<'a>,
}
inventory::collect!(ImplEntry<'static>);

impl ImplEntry<'_> {
    #[doc(hidden)]
    pub fn new<T: TypeOid + RegisteredImpl>() -> Self {
        Self {
            id: ImplId::new::<T>(),
            data: ImplData {
                vtable: <T as RegisteredImpl>::vtable(),
                name: <T as TypeOid>::type_name(),
                desc: <T as TypeOid>::type_desc(),
                info: <T as RegisteredImpl>::debug_info(),
            },
        }
    }
}

#[derive(Debug)]
struct ImplRegistry {
    oid_to_data: HashMap<ImplId<'static>, ImplData<'static>>,
}

impl ImplRegistry {
    fn new() -> Self {
        Self {
            oid_to_data: HashMap::new(),
        }
    }

    fn add_entry(&mut self, entry: &'static ImplEntry) {
        let old_val = self.oid_to_data.insert(entry.id, entry.data);

        if let Some(old) = old_val {
            eprintln!("Value impl oid conflict for {:?}", entry.id.type_oid);
            eprintln!(
                "Existing impl registered at {}:{}:{}",
                old.info.file, old.info.line, old.info.column
            );
            eprintln!(
                "New impl registered at {}:{}:{}",
                entry.data.info.file, entry.data.info.line, entry.data.info.column
            );
        }
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
    fn debug_info() -> ImplDebugInfo;
}

#[macro_use]
pub mod macros {
    #[macro_export]
    macro_rules! debug_info {
        () => {
            $crate::resources::state::value::ImplDebugInfo {
                file: ::core::file!(),
                line: ::core::line!(),
                column: ::core::column!(),
            }
        };
    }
    #[macro_export]
    macro_rules! oiddeser {
        ( $y:ty, $z:ty ) => {
            impl $crate::resources::state::value::DeserializeDynOid for $y
            where
                $y: for<'a> ::rkyv::Deserialize<$z, (dyn ::rkyv_dyn::DynDeserializer + 'a)>,
            {
                unsafe fn deserialize_dynoid(
                    &self,
                    deserializer: &mut dyn ::rkyv_dyn::DynDeserializer,
                    alloc: &mut dyn FnMut(::core::alloc::Layout) -> *mut u8,
                ) -> Result<*mut (), ::rkyv_dyn::DynError> {
                    let ptr = alloc(::core::alloc::Layout::new::<$z>()).cast::<$z>();
                    ptr.write(self.deserialize(deserializer)?);
                    Ok(ptr as *mut ())
                }

                fn deserialize_dynoid_metadata(
                    &self,
                    _: &mut dyn ::rkyv_dyn::DynDeserializer,
                ) -> ::std::result::Result<<dyn $crate::resources::state::value::SerializeValue
                as
                ::ptr_meta::Pointee>::Metadata, ::rkyv_dyn::DynError> {
                    unsafe {
                        Ok(core::mem::transmute(ptr_meta::metadata(
                            ::core::ptr::null::<$z>() as *const dyn $crate::resources::state::value::SerializeValue,
                        )))
                    }
                }
            }
        };
    }
    #[macro_export]
    macro_rules! oidvalue {
        ( $x:ident, $y:ty ) => {
            $crate::oidvalue! {$x, $y, $y}
        };
        ( $x:ident, $y:ty, $z:ty ) => {
            $crate::oiddeser! {$z, $y}

            impl $crate::resources::state::value::TypeOid for $z {
                fn type_oid() -> &'static $crate::utils::oid::ObjectIdentifier {
                    &$x
                }

                fn type_name() -> &'static str {
                    stringify!($y)
                }

                fn type_desc() -> &'static str {
                    "builtin"
                }
            }
            unsafe impl $crate::resources::state::value::RegisteredImpl for $z {
                fn vtable() -> usize {
                    unsafe {
                        ::core::mem::transmute(ptr_meta::metadata(
                            ::core::ptr::null::<$z>() as *const dyn $crate::resources::state::value::DeserializeValue
                        ))
                    }
                }
                fn debug_info() -> $crate::resources::state::value::ImplDebugInfo {
                    $crate::debug_info!()
                }
            }

            ::inventory::submit! {$crate::resources::state::value::ImplEntry::new::<$z>()}
        };
    }
}
use macros::*;

lazy_static::lazy_static! {
    pub static ref OID_BOOL: ObjectIdentifier = {
        ObjectIdentifier::from_str("1.3.6.1.4.1.48398.612.1.1").unwrap()
    };
    pub static ref OID_U8: ObjectIdentifier = {
        ObjectIdentifier::from_str("1.3.6.1.4.1.48398.612.1.2").unwrap()
    };
    pub static ref OID_U16: ObjectIdentifier = {
        ObjectIdentifier::from_str("1.3.6.1.4.1.48398.612.1.3").unwrap()
    };
    pub static ref OID_U32: ObjectIdentifier = {
        ObjectIdentifier::from_str("1.3.6.1.4.1.48398.612.1.4").unwrap()
    };
    pub static ref OID_U64: ObjectIdentifier = {
        ObjectIdentifier::from_str("1.3.6.1.4.1.48398.612.1.5").unwrap()
    };
    pub static ref OID_U128: ObjectIdentifier = {
        ObjectIdentifier::from_str("1.3.6.1.4.1.48398.612.1.6").unwrap()
    };
    pub static ref OID_I8: ObjectIdentifier = {
        ObjectIdentifier::from_str("1.3.6.1.4.1.48398.612.1.7").unwrap()
    };
    pub static ref OID_I16: ObjectIdentifier = {
        ObjectIdentifier::from_str("1.3.6.1.4.1.48398.612.1.8").unwrap()
    };
    pub static ref OID_I32: ObjectIdentifier = {
        ObjectIdentifier::from_str("1.3.6.1.4.1.48398.612.1.9").unwrap()
    };
    pub static ref OID_I64: ObjectIdentifier = {
        ObjectIdentifier::from_str("1.3.6.1.4.1.48398.612.1.10").unwrap()
    };
    pub static ref OID_I128: ObjectIdentifier = {
        ObjectIdentifier::from_str("1.3.6.1.4.1.48398.612.1.11").unwrap()
    };
    pub static ref OID_VEC3U8: ObjectIdentifier = {
        ObjectIdentifier::from_str("1.3.6.1.4.1.48398.612.1.13").unwrap()
    };

    pub static ref OID_POWERED: ObjectIdentifier = {
        ObjectIdentifier::from_str("1.3.6.1.4.1.48398.612.2.1").unwrap()
    };
    pub static ref OID_INTENSITY: ObjectIdentifier = {
        ObjectIdentifier::from_str("1.3.6.1.4.1.48398.612.2.2").unwrap()
    };
    pub static ref OID_COLOUR: ObjectIdentifier = {
        ObjectIdentifier::from_str("1.3.6.1.4.1.48398.612.2.3").unwrap()
    };
}
oidvalue!(OID_BOOL, bool);
oidvalue!(OID_U8, u8);
oidvalue!(OID_U16, u16);
oidvalue!(OID_U32, u32);
oidvalue!(OID_U64, u64);
oidvalue!(OID_U128, u128);
oidvalue!(OID_I8, i8);
oidvalue!(OID_I16, i16);
oidvalue!(OID_I32, i32);
oidvalue!(OID_I64, i64);
oidvalue!(OID_I128, i128);

#[derive(
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    Hash,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[archive_attr(derive(Debug, PartialEq, serde::Serialize, serde::Deserialize))]
pub struct Vec3u8 {
    pub a: u8,
    pub b: u8,
    pub c: u8,
}
oidvalue!(OID_VEC3U8, Vec3u8, ArchivedVec3u8);

#[cfg(test)]
mod tests {
    use super::*;
    use rand::distributions::Standard;
    use rand::prelude::Distribution;
    use rand::Rng;

    impl Distribution<Vec3u8> for Standard {
        fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Vec3u8 {
            let a = self.sample(rng);
            let b = self.sample(rng);
            let c = self.sample(rng);
            Vec3u8 { a, b, c }
        }
    }
}
