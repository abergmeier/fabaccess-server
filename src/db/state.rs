use std::{
    fmt,

    any::Any,

    collections::{
        hash_map::DefaultHasher
    },
    hash::{
        Hash, 
        Hasher
    },

    path::Path,
};

use rkyv::{
    Archive,
    Archived,

    Serialize,
    Deserialize,

    out_field,

    Fallible,
    ser::serializers::AllocSerializer,
};
use rkyv_dyn::{
    archive_dyn,
};
use rkyv_typename::TypeName;

use crate::db::{
    DB,
    Environment,

    EnvironmentFlags,
    DatabaseFlags,
    WriteFlags,

    Adapter,

    Transaction,
    RwTransaction,
};

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

struct StateAdapter;

enum StateError {
    LMDB(lmdb::Error),
    RKYV(<AllocSerializer<1024> as Fallible>::Error),
}

impl From<lmdb::Error> for StateError {
    fn from(e: lmdb::Error) -> Self {
        Self::LMDB(e)
    }
}

impl Fallible for StateAdapter {
    type Error = StateError;
}
impl Adapter for StateAdapter {
    type Serializer = AllocSerializer<1024>;
    type Value = State;

    fn new_serializer() -> Self::Serializer {
        Self::Serializer::default()
    }

    fn from_ser_err(e: <Self::Serializer as Fallible>::Error) -> Self::Error {
        StateError::RKYV(e)
    }
    fn from_db_err(e: lmdb::Error) -> Self::Error {
        e.into()
    }
}

/// State Database containing the currently set state
pub struct StateDB {
    /// The environment for all the databases below
    env: Environment,

    input: DB<StateAdapter>,
    output: DB<StateAdapter>,

    // TODO: Index resource name/id/uuid -> u64
}

impl StateDB {
    fn open_env<P: AsRef<Path>>(path: &P) -> lmdb::Result<Environment> {
        Environment::new()
            .set_flags( EnvironmentFlags::WRITE_MAP 
                      | EnvironmentFlags::NO_SUB_DIR 
                      | EnvironmentFlags::NO_TLS
                      | EnvironmentFlags::NO_READAHEAD)
            .set_max_dbs(2)
            .open(path.as_ref())
    }

    fn new(env: Environment, input: DB<StateAdapter>, output: DB<StateAdapter>) -> Self {
        Self { env, input, output }
    }

    pub fn init<P: AsRef<Path>>(path: &P) -> lmdb::Result<Self> {
        let env = Self::open_env(path)?;
        let input = unsafe {
            DB::create(&env, Some("input"), DatabaseFlags::INTEGER_KEY)?
        };
        let output = unsafe {
            DB::create(&env, Some("output"), DatabaseFlags::INTEGER_KEY)?
        };

        Ok(Self::new(env, input, output))
    }

    pub fn open<P: AsRef<Path>>(path: &P) -> lmdb::Result<Self> {
        let env = Self::open_env(path)?;
        let input = unsafe { DB::open(&env, Some("input"))?  };
        let output = unsafe { DB::open(&env, Some("output"))?  };

        Ok(Self::new(env, input, output))
    }

    fn update_txn(&self, txn: &mut RwTransaction, key: u64, input: &State, output: &State)
        -> Result<(), <StateAdapter as Fallible>::Error>
    {
        let flags = WriteFlags::empty();
        let k = key.to_ne_bytes();
        self.input.put(txn, &k, input, flags)?;
        self.output.put(txn, &k, output, flags)?;
        Ok(())
    }

    fn update(&self, key: u64, input: &State, output: &State) 
        -> Result<(), <StateAdapter as Fallible>::Error>
    {
        let mut txn = self.env.begin_rw_txn().map_err(StateAdapter::from_db_err)?;
        self.update_txn(&mut txn, key, input, output)?;

        txn.commit().map_err(StateAdapter::from_db_err)
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
