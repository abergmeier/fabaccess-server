mod raw;
pub use raw::RawDB;

mod typed;
pub use typed::{DB, ArchivedValue, Adapter, AlignedAdapter};

pub type Error = lmdb::Error;