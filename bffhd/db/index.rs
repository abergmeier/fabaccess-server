use std::{fs};
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{RwLock};
use anyhow::Context;

use rkyv::{Archive, Serialize, Deserialize, AlignedVec, Archived, with::Lock, Fallible};
use rkyv::de::deserializers::SharedDeserializeMap;
use rkyv::ser::Serializer;
use rkyv::ser::serializers::{AlignedSerializer, AllocScratch, AllocScratchError, AllocSerializer, CompositeSerializer, CompositeSerializerError, FallbackScratch, HeapScratch, ScratchTracker, SharedSerializeMap, SharedSerializeMapError};
use rkyv::with::LockError;

pub trait Index {
    type Key: ?Sized;

    fn lookup(&self, key: &Self::Key) -> Option<u64>;
    fn update(&mut self, key: &Self::Key, value: u64);
}

pub struct StringIndex {
    inner: HashMap<String, u64>,
}

impl Index for StringIndex {
    type Key = str;

    fn lookup(&self, key: &Self::Key) -> Option<u64> {
        self.inner.get(key).map(|v| *v)
    }

    fn update(&mut self, key: &Self::Key, new: u64) {
        let old = self.inner.insert(key.to_string(), new);
        tracing::trace!(key, ?old, new, "updated string index");
    }
}

#[derive(Debug, Archive, Serialize, Deserialize)]
pub struct DbIndexManager<I> {
    name: String,

    // TODO: use locking? Write are serialized anyway
    generation: AtomicU64,
    next_id: AtomicU64,

    #[with(Lock)]
    indices: RwLock<I>,
}

type S = CompositeSerializer<AlignedSerializer<AlignedVec>,
    ScratchTracker<FallbackScratch<HeapScratch<1024>, AllocScratch>>, SharedSerializeMap>;
type SE = CompositeSerializerError<std::convert::Infallible, AllocScratchError, SharedSerializeMapError>;
#[derive(Debug)]
pub struct Ser (pub(super) S);
impl Default for Ser {
    fn default() -> Self {
        Self(CompositeSerializer::new(AlignedSerializer::default(), ScratchTracker::new(FallbackScratch::default()), SharedSerializeMap::default()))
    }
}

#[derive(Debug)]
pub enum SerError {
    Composite(SE),
    Lock(LockError),
}
impl Display for SerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Composite(e) => Display::fmt(e, f),
            Self::Lock(e) => Display::fmt(e, f),
        }
    }
}
impl std::error::Error for SerError {}

impl From<SE> for SerError {
    fn from(e: SE) -> Self {
        Self::Composite(e)
    }
}
impl From<LockError> for SerError {
    fn from(e: LockError) -> Self {
        Self::Lock(e)
    }
}
impl Fallible for Ser { type Error = SerError; }
impl Serializer for Ser {
    fn pos(&self) -> usize {
        self.0.pos()
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        self.0.write(bytes).map_err(|e| e.into())
    }
}

type Ser2 = AllocSerializer<4096>;

impl<I> DbIndexManager<I> {
    pub fn new(name: String, generation: u64, next_id: u64, indices: I) -> Self {
        tracing::debug!(%name, generation, next_id, "constructing db index");
        Self {
            name,
            generation: AtomicU64::new(generation),
            next_id: AtomicU64::new(next_id),
            indices: RwLock::new(indices),
        }
    }
}

impl<I> DbIndexManager<I>
    where I: 'static + Archive + Serialize<Ser>,
          <I as Archive>::Archived: Deserialize<I, SharedDeserializeMap>,
{
    pub fn store(&self, path: impl AsRef<Path>) -> anyhow::Result<usize> {
        let path = path.as_ref();

        let span = tracing::debug_span!("store",
            name=%self.name, path=%path.display(),
            "storing database index"
        );
        let _guard = span.enter();

        tracing::trace!("opening db index file");
        let mut fd = fs::File::create(path)
            .with_context(|| format!("failed to open database index file {}", path.display()))?;
        tracing::trace!(?fd, "opened db index file");

        let mut serializer = Ser::default();
        tracing::trace!(?serializer, "serializing db index");
        let root = serializer.serialize_value(self).context("serializing database index failed")?;
        let (s, c, _h) = serializer.0.into_components();
        let v = s.into_inner();
        tracing::trace!(%root,
            len = v.len(),
            max_bytes_allocated = c.max_bytes_allocated(),
            max_allocations = c.max_allocations(),
            max_alignment = c.max_alignment(),
            min_buffer_size = c.min_buffer_size(),
            min_buffer_size_max_error = c.min_buffer_size_max_error(),
            "serialized db index");

        let () = fd.write_all(v.as_slice())
            .with_context(|| format!("failed to write {} bytes to database index file at {}", v.len(), path.display()))?;

        Ok(v.len())
    }

    pub fn load<'a>(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref();

        let span = tracing::debug_span!("load",
            path=%path.display(),
            "loading database index"
        );
        let _guard = span.enter();

        tracing::trace!("reading db index file");
        let data = fs::read(path).with_context(|| format!("failed to read database index file at {}", path.display()))?;
        tracing::trace!(len=data.len(), "read db index file");

        let res = unsafe {
            let maybe_this: &Archived<Self> = rkyv::archived_root::<Self>(&data[..]);
            // TODO: validate `maybe_this`
            maybe_this
        };
        tracing::trace!("loaded db index from file");

        let mut deser = SharedDeserializeMap::default();
        let this: Self = Deserialize::<Self, _>::deserialize(res, &mut deser)?;

        tracing::trace!(generation=this.generation.load(Ordering::Relaxed),
            "deserialized db index from file");

        Ok(this)
    }

    /// Return a new unused ID using an atomic fetch-add
    pub fn get_next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Release)
    }
}
impl<I: Index> DbIndexManager<I> {
    pub fn lookup(&self, key: &I::Key) -> Option<u64> {
        self.indices.read().unwrap().lookup(key)
    }

    pub fn update(&self, key: &I::Key, value: u64) {
        self.indices.write().unwrap().update(key, value)
    }
}