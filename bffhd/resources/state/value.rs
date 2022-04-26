use std::{hash::Hash};

use ptr_meta::{DynMetadata, Pointee};
use rkyv::{out_field, Archive, ArchivePointee, ArchiveUnsized, Archived, ArchivedMetadata, Serialize, SerializeUnsized, RelPtr};
use rkyv_dyn::{DynError, DynSerializer};


use crate::utils::oid::ObjectIdentifier;

// Not using linkme because dynamically loaded modules
use inventory;

use rkyv::ser::{ScratchSpace, Serializer};

use serde::ser::SerializeMap;

use std::collections::HashMap;


use std::ops::Deref;
use std::sync::atomic::{AtomicUsize, Ordering};

use rkyv::vec::ArchivedVec;

#[repr(transparent)]
struct MetaBox<T: ?Sized>(Box<T>);
impl<T: ?Sized> From<Box<T>> for MetaBox<T> {
    fn from(b: Box<T>) -> Self {
        Self(b)
    }
}

#[repr(transparent)]
struct ArchivedMetaBox<T: ArchivePointee + ?Sized>(RelPtr<T>);
impl<T: ArchivePointee + ?Sized> ArchivedMetaBox<T> {
    #[inline]
    pub fn get(&self) -> &T {
        unsafe { &*self.0.as_ptr() }
    }
}

impl<T: ArchivePointee + ?Sized> AsRef<T> for ArchivedMetaBox<T> {
    fn as_ref(&self) -> &T {
        self.get()
    }
}

impl<T: ArchivePointee + ?Sized> Deref for ArchivedMetaBox<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

// State built as
struct NewStateBuilder {
    inner: Vec<MetaBox<dyn SerializeStateValue>>,
}

// turns into
struct NewState {
    inner: ArchivedVec<ArchivedMetaBox<dyn ArchivedStateValue>>
}
impl NewState {
    pub fn get_value<T: TypeOid>(&self) -> Option<&T> {
        /*
        let target_oid = T::type_oid();

        let values = self.inner.as_slice();
        for v in values {
            let oid: &Archived<ObjectIdentifier> = &v.metadata().type_oid;
            if &target_oid.deref() == &oid.deref() {
                let value = unsafe { &*v.as_ptr().cast() };
                return Some(value);
            }
        }

        None
         */
        unimplemented!()
    }
}

// for usage.
// The important part is that both `SerializeValue` and `Value` tell us their OIDs. State will
// usually consist of only a very small number of parts, most of the time just one, so linear
// search will be the best.
// `dyn Value` is Archived using custom sauce Metadata that will store the OID of the state
// value, allowing us to cast the object (reasonably) safely. Thus we can also add a
// method `get_part<T: Value>(&self) -> Option<&T>`
// ArchivedBox is just a RelPtr into the object; so we'd use an `ArchivedValue<NewDesignState>`.
// We can freely modify the memory of the value, so caching vtables is possible & sensible?
// For dumping / loading values using serde we have to be able to serialize a `dyn Value` and to
// deserialize a `dyn SerializeValue`.
// This means, for every type T that's a value we must have:
// - impl SerializeValue for T, which probably implies impl Value for T?
// - impl Value for Archived<T>
// - impl serde::Deserialize for T
// - impl serde::Serialize for Archived<T>
// - impl rkyv::Archive, rkyv::Serialize for T

#[ptr_meta::pointee]
/// Trait that values in the State Builder have to implement
///
/// It requires serde::Deserialize and rkyv::SerializeUnsized to be implemented.
///
/// it is assumed that there is a 1:1 mapping between a SerializeStateValue and a StateValue
/// implementation. Every `T` implementing the former has exactly *one* `Archived<T>` implementing
/// the latter.
///
/// The archived version of any implementation must implement [ArchivedStateValue](trait@ArchivedStateValue).
pub trait SerializeStateValue: SerializeDynOid {}

#[ptr_meta::pointee]
/// Trait that (rkyv'ed) values in the State Object have to implement.
///
/// It requires serde::Serialize to be implemented.
///
/// It must be Sync since the State is sent as a signal to all connected actors by reference.
/// It must be Send since the DB thread and the signal thread may be different.
pub trait ArchivedStateValue: Send + Sync {}

/// Serializing a trait object by storing an OID alongside
///
/// This trait is a dependency for [SerializeStateValue](trait@SerializeStateValue). It is by
/// default implemented for all `T where T: for<'a> Serialize<dyn DynSerializer + 'a>, T::Archived: TypeOid`.
pub trait SerializeDynOid {
    /// Return the OID associated with the **Archived** type, i.e. `Archived<Self>`.
    ///
    /// This OID will be serialized alongside the trait object and is used to retrieve the
    /// correct vtable when loading the state from DB.
    fn archived_type_oid(&self) -> &'static ObjectIdentifier;

    /// Serialize this type into a [`DynSerializer`](trait@DynSerializer)
    fn serialize_dynoid(&self, serializer: &mut dyn DynSerializer) -> Result<usize, DynError>;
}

/// Types with an associated OID
///
/// This trait is required by the default implementation of [SerializeDynOid](trait@SerializeDynOid),
/// providing the OID that is serialized alongside the state object to be able to correctly cast
/// it when accessing state from the DB.
pub trait TypeOid {
    fn type_oid() -> &'static ObjectIdentifier;
    fn type_name() -> &'static str;
}

impl<T> SerializeDynOid for T
    where
        T: for<'a> Serialize<dyn DynSerializer + 'a>,
        T::Archived: TypeOid,
{
    fn archived_type_oid(&self) -> &'static ObjectIdentifier {
        Archived::<T>::type_oid()
    }

    fn serialize_dynoid(&self, serializer: &mut dyn DynSerializer) -> Result<usize, DynError> {
        serializer.serialize_value(self)
    }
}

impl ArchivePointee for dyn ArchivedStateValue {
    type ArchivedMetadata = ArchivedStateValueMetadata;

    fn pointer_metadata(archived: &Self::ArchivedMetadata) -> <Self as Pointee>::Metadata {
        archived.pointer_metadata()
    }
}

impl ArchiveUnsized for dyn SerializeStateValue {
    type Archived = dyn ArchivedStateValue;
    type MetadataResolver = <ObjectIdentifier as Archive>::Resolver;

    unsafe fn resolve_metadata(
        &self,
        pos: usize,
        resolver: Self::MetadataResolver,
        out: *mut ArchivedMetadata<Self>, // => ArchivedStateValueMetadata
    ) {
        let (oid_pos, oid) = out_field!(out.type_oid);
        let type_oid = self.archived_type_oid();
        type_oid.resolve(pos + oid_pos, resolver, oid);

        let (_vtable_cache_pos, vtable_cache) = out_field!(out.vtable_cache);
        *vtable_cache = AtomicUsize::default();
    }
}

impl<S: ScratchSpace + Serializer + ?Sized> SerializeUnsized<S> for dyn SerializeStateValue {
    fn serialize_unsized(&self, mut serializer: &mut S) -> Result<usize, S::Error> {
        self.serialize_dynoid(&mut serializer)
            .map_err(|e| *e.downcast::<S::Error>().unwrap())
    }

    fn serialize_metadata(&self, serializer: &mut S) -> Result<Self::MetadataResolver, S::Error> {
        let oid = self.archived_type_oid();
        oid.serialize(serializer)
    }
}

#[derive(Debug)]
pub struct ArchivedStateValueMetadata {
    pub type_oid: Archived<ObjectIdentifier>,
    vtable_cache: AtomicUsize,
}

impl ArchivedStateValueMetadata {
    // TODO: `usize as *const VTable` is not sane.
    pub fn vtable(&self) -> usize {
        let val = self.vtable_cache.load(Ordering::Relaxed);
        if val != 0 {
            return val;
        }

        let val = IMPL_REGISTRY
            .get(ImplId::from_type_oid(&self.type_oid))
            .expect(&format!("Unregistered type oid {:?}", self.type_oid))
            .vtable;
        self.vtable_cache.store(val, Ordering::Relaxed);
        return val;
    }

    pub fn pointer_metadata(&self) -> DynMetadata<dyn ArchivedStateValue> {
        unsafe { core::mem::transmute(self.vtable()) }
    }
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
/// OID of an [ArchivedStateValue](trait@ArchivedStateValue) implementation.
///
/// Used by the global type registry of all implementations to look up the vtables of state values
/// when accessing it from DB and when (de-)serializing it using serde.
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
        Self {
            type_oid: &T::type_oid(),
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct ImplData<'a> {
    pub vtable: usize,
    pub name: &'a str,
    pub info: ImplDebugInfo,
}

#[derive(Copy, Clone, Debug)]
pub struct ImplDebugInfo {
    pub file: &'static str,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug)]
/// State Value Implementation Entry
///
/// To register a state implementation you must call [inventory::collect](macro@inventory::collect)
/// macro for an Entry constructed for your type on top level. Your type will have to have
/// implementations of [TypeOid](trait@TypeOid) and [RegisteredImpl](trait@RegisteredImpl)
/// Alternatively you can use the
/// [statevalue_register](macro@crate::statevalue_register) macro with your OID as first and type
/// as second parameter like so:
///
/// ```no_run
/// struct MyStruct;
/// statevalue_register!(ObjectIdentifier::from_str("1.3.6.1.4.1.48398.612.1.14").unwrap(), MyStruct)
/// ```
pub struct ImplEntry<'a> {
    id: ImplId<'a>,
    data: ImplData<'a>,
}
inventory::collect!(ImplEntry<'static>);

impl ImplEntry<'_> {
    pub fn new<T: TypeOid + RegisteredImpl>() -> Self {
        Self {
            id: ImplId::new::<T>(),
            data: ImplData {
                vtable: <T as RegisteredImpl>::vtable(),
                name: <T as TypeOid>::type_name(),
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

#[doc(hidden)]
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
    macro_rules! statevalue_typeoid {
        ( $x:ident, $y:ty, $z:ty ) => {
            impl $crate::resources::state::value::TypeOid for $z {
                fn type_oid() -> &'static $crate::utils::oid::ObjectIdentifier {
                    &$x
                }

                fn type_name() -> &'static str {
                    stringify!($y)
                }
            }
        }
    }

    #[macro_export]
    macro_rules! statevalue_registeredimpl {
        ( $z:ty ) => {
            unsafe impl $crate::resources::state::value::RegisteredImpl for $z {
                fn vtable() -> usize {
                    unsafe {
                        ::core::mem::transmute(ptr_meta::metadata(
                            ::core::ptr::null::<$z>() as *const dyn $crate::resources::state::value::ArchivedStateValue
                        ))
                    }
                }
                fn debug_info() -> $crate::resources::state::value::ImplDebugInfo {
                    $crate::debug_info!()
                }
            }
        }
    }

    #[macro_export]
    macro_rules! statevalue_register {
        ( $x:ident, $y:ty ) => {
            $crate::oidvalue! {$x, $y, $y}
        };
        ( $x:ident, $y:ty, $z:ty ) => {
            $crate::statevalue_typeoid! { $x, $y, $z }
            $crate::statevalue_registeredimpl! { $z }

            ::inventory::submit! {$crate::resources::state::value::ImplEntry::new::<$z>()}
        };
    }
}

/*
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
}

impl<T: Archive + Value + SerializeDynOid + Clone> SerializeValue for T
where
    T::Archived: RegisteredImpl,
{
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

 */