mod raw;
pub use raw::RawDB;

mod typed;
pub use typed::{Adapter, AlignedAdapter, ArchivedValue, DB};

pub type Error = lmdb::Error;
