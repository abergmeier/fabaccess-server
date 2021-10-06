use std::fmt;
use std::rc::Rc;
use std::sync::Arc;
use std::any::Any;
use std::collections::{HashMap, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::default::Default;
use std::ptr::NonNull;
use std::alloc::Layout;
use std::ops::Deref;

use rkyv::{
    Archive,
    Archived,

    Serialize,
    Deserialize,

    Fallible,
    ser::{
        Serializer,
        ScratchSpace,
        serializers::*,
    },

    string::{
        StringResolver,
        ArchivedString,
    },

    out_field,
    archived_root,
};
use rkyv_dyn::{
    archive_dyn,
};
use rkyv_typename::TypeName;

use crate::error::Error;
use crate::db::{DB, Environment, WriteFlags, Transaction, RoTransaction};

#[archive_dyn(deserialize)]
/// Trait to be implemented by any value in the state map.
///
/// A value can be any type not having dangling references (with the added restriction that it has
/// to implement `Debug` for debugger QoL).
/// In fact Value *also* needs to implement Hash since BFFH checks if the state is different to
/// before on input and output before updating the resource re. notifying actors and notifys.  This
/// dependency is not expressable via supertraits since it is not possible to make Hash into a
/// trait object.
/// To solve this [`State`] uses the [`StateBuilder`] which adds an `Hash` requirement for inputs
/// on [`add`](struct::StateBuilder::add). The hash is being created over all inserted values and
/// then used to check for equality. Note that in addition to collisions, Hash is not guaranteed
/// stable over ordering and will additionally not track overwrites, so if the order of insertions
/// changes or values are set and later overwritten then two equal States can and are likely to
/// have different hashes.
pub trait Value: Any + fmt::Debug { }

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Archive, Serialize, Deserialize)]
#[archive_attr(derive(TypeName, Debug))]
pub struct Bool(bool);

#[archive_dyn(deserialize)]
impl Value for Bool { }
impl Value for Archived<Bool> { }

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Archive, Serialize, Deserialize)]
#[archive_attr(derive(TypeName, Debug))]
pub struct UInt32(u32);

#[archive_dyn(deserialize)]
impl Value for UInt32 { }
impl Value for Archived<UInt32> { }

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Archive, Serialize, Deserialize)]
#[archive_attr(derive(TypeName, Debug))]
pub struct Vec3u8 {
    a: u8,
    b: u8,
    c: u8,
}

#[archive_dyn(deserialize)]
impl Value for Vec3u8 { }
impl Value for Archived<Vec3u8> { }

#[derive(Archive, Serialize, Deserialize)]
/// State object of a resource
///
/// This object serves three functions:
/// 1. it is constructed by modification via Claims or via internal resource logic
/// 2. it is serializable and storable in the database
/// 3. it is sendable and forwarded to all Actors and Notifys
pub struct State {
    hash: u64,
    inner: Vec<(String, Box<dyn SerializeValue>)>,
}
impl PartialEq for State {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}
impl Eq for State {}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut sf = f.debug_struct("State");
        for (k, v) in self.inner.iter() {
            sf.field(k, v);
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
    inner: Vec<(String, Box<dyn SerializeValue>)>
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
    pub fn add<V>(mut self, key: String, val: V) -> Self 
        where V: SerializeValue + Hash
    {
        key.hash(&mut self.hasher);
        val.hash(&mut self.hasher);

        self.inner.push((key, Box::new(val)));

        self
    }
}

pub struct StateStorage {
    key: u64,
    db: StateDB
}

impl StateStorage {
    pub fn new(key: u64, db: StateDB) -> Self {
        Self { key, db }
    }

    pub fn store(&mut self, instate: &State, outstate: &State) -> Result<(), Error> {
        self.db.store(self.key, instate, outstate)
    }
}

struct SizeSerializer {
    pos: usize,
    scratch: FallbackScratch<HeapScratch<1024>, AllocScratch>,
}
impl SizeSerializer {
    pub fn new() -> Self {
        Self { pos: 0, scratch: FallbackScratch::default() }
    }
}
impl Fallible for SizeSerializer {
    type Error = AllocScratchError;
}
impl Serializer for SizeSerializer {
    fn pos(&self) -> usize {
        self.pos
    }
    fn write(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        self.pos += bytes.len();
        Ok(())
    }
}
impl ScratchSpace for SizeSerializer {
    unsafe fn push_scratch(
        &mut self,
        layout: Layout
    ) -> Result<NonNull<[u8]>, Self::Error> {
        self.scratch.push_scratch(layout)
    }

    unsafe fn pop_scratch(
        &mut self, 
        ptr: NonNull<u8>, 
        layout: Layout
    ) -> Result<(), Self::Error> {
        self.scratch.pop_scratch(ptr, layout)
    }
}

type LmdbSerializer<B, const N: usize> = CompositeSerializer<
    BufferSerializer<B>, 
    FallbackScratch<HeapScratch<N>, AllocScratch>, 
    SharedSerializeMap,
>;


pub struct StateDB {
    input: DB,
    output: DB,
}

impl StateDB {
    pub fn new(input: DB, output: DB) -> Self {
        Self { input, output }
    }

    fn get_size(&self, state: &State) -> usize {
        let mut serializer = SizeSerializer::new();
        serializer.serialize_value(state);
        serializer.pos()
    }

    pub fn store(&self, key: u64, instate: &State, outstate: &State) -> Result<(), Error> {
        let insize = self.get_size(instate);
        let outsize = self.get_size(outstate);

        let mut txn = self.input.begin_rw_txn()?;

        let mut inbuf = self.input.reserve(&mut txn, &key.to_ne_bytes(), insize, WriteFlags::empty())?;
        let bufser = BufferSerializer::new(inbuf);
        let ser: LmdbSerializer<&mut [u8], 1024> = LmdbSerializer::new(
            bufser,
            FallbackScratch::default(),
            SharedSerializeMap::default()
        );

        let mut outbuf = self.output.reserve(&mut txn, &key.to_ne_bytes(), outsize, WriteFlags::empty())?;
        let bufser = BufferSerializer::new(outbuf);
        let ser: LmdbSerializer<&mut [u8], 1024> = LmdbSerializer::new(
            bufser,
            FallbackScratch::default(),
            SharedSerializeMap::default()
        );

        txn.commit()?;

        Ok(())
    }

    pub fn get_txn<'txn, T: Transaction>(&self, key: u64, txn: &'txn T)
        -> Result<(&'txn ArchivedState, &'txn ArchivedState), Error> 
    {
        let inbuf = self.input.get(txn, &key.to_ne_bytes())?;
        let outbuf = self.output.get(txn, &key.to_ne_bytes())?;
        let instate = unsafe {
            archived_root::<State>(inbuf.as_ref())
        };
        let outstate = unsafe {
            archived_root::<State>(outbuf.as_ref())
        };

        Ok((instate, outstate))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::db::tests::open_test_env;
    use lmdb::{
        EnvironmentFlags as EF,
        DatabaseFlags as DF,
        WriteFlags as WF,
    };

    use rkyv::Infallible;
    use rkyv::ser::serializers::AllocSerializer;
    use rkyv::archived_root;
    use rkyv::util::archived_value;

    #[test]
    fn construct_state() {
        let b = State::build()
            .add("Colour".to_string(), Vec3u8 { a: 1, b: 2, c: 3})
            .add("Powered".to_string(), Bool(true))
            .add("Intensity".to_string(), UInt32(4242))
            .finish();

        println!("({}) {:?}", b.hash(), b);

        let mut serializer = AllocSerializer::<256>::default();
        let pos = serializer.serialize_value(&b).unwrap();
        let buf = serializer.into_serializer().into_inner();

        println!("Encsize: {}", buf.len());

        let archived_state = unsafe {
            archived_value::<State>(buf.as_ref(), pos)
        };
        let s: State = archived_state.deserialize(&mut Infallible).unwrap();

        println!("({}) {:?}", pos, s);
    }

    #[test]
    fn function_name_test() {
        let te = open_text_env();
        let ildb = e.create_db(Some("input"), DF::empty()).expect("Failed to create db file");
        let oldb = e.create_db(Some("output"), DF::empty()).expect("Failed to create db file");

        let idb = DB::new(e.env.clone(), ildb);
        let odb = DB::new(e.env.clone(), oldb);
        let db = StateDB::new(idb, odb);
    }
}
