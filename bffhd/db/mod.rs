use thiserror::Error;

// for converting a database error into a failed promise
use capnp;

mod raw;

use miette::{Diagnostic, Severity};
pub use raw::RawDB;
use std::fmt::{Debug, Display};

mod typed;
pub use typed::{Adapter, AlignedAdapter, ArchivedValue, DB};

pub type ErrorO = lmdb::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialEq, Eq, Error)]
#[error(transparent)]
#[repr(transparent)]
pub struct Error(#[from] lmdb::Error);

impl Diagnostic for Error {
    fn code<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        Some(Box::new(match self.0 {
            lmdb::Error::KeyExist => "bffh::db::raw::key_exists".to_string(),
            lmdb::Error::NotFound => "bffh::db::raw::not_found".to_string(),
            lmdb::Error::PageNotFound => "bffh::db::raw::page_not_found".to_string(),
            lmdb::Error::Corrupted => "bffh::db::raw::corrupted".to_string(),
            lmdb::Error::Panic => "bffh::db::raw::panic".to_string(),
            lmdb::Error::VersionMismatch => "bffh::db::raw::version_mismatch".to_string(),
            lmdb::Error::Invalid => "bffh::db::raw::invalid".to_string(),
            lmdb::Error::MapFull => "bffh::db::raw::map_full".to_string(),
            lmdb::Error::DbsFull => "bffh::db::raw::dbs_full".to_string(),
            lmdb::Error::ReadersFull => "bffh::db::raw::readers_full".to_string(),
            lmdb::Error::TlsFull => "bffh::db::raw::tls_full".to_string(),
            lmdb::Error::TxnFull => "bffh::db::raw::txn_full".to_string(),
            lmdb::Error::CursorFull => "bffh::db::raw::cursor_full".to_string(),
            lmdb::Error::PageFull => "bffh::db::raw::page_full".to_string(),
            lmdb::Error::MapResized => "bffh::db::raw::map_resized".to_string(),
            lmdb::Error::Incompatible => "bffh::db::raw::incompatible".to_string(),
            lmdb::Error::BadRslot => "bffh::db::raw::bad_rslot".to_string(),
            lmdb::Error::BadTxn => "bffh::db::raw::bad_txn".to_string(),
            lmdb::Error::BadValSize => "bffh::db::raw::bad_val_size".to_string(),
            lmdb::Error::BadDbi => "bffh::db::raw::bad_dbi".to_string(),
            lmdb::Error::Other(n) => format!("bffh::db::raw::e{}", n),
        }))
    }

    fn severity(&self) -> Option<Severity> {
        Some(Severity::Error)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        match self.0 {
            lmdb::Error::KeyExist => Some(Box::new("The provided key already exists in the database")),
            lmdb::Error::NotFound => Some(Box::new("The requested key was not found in the database")),
            lmdb::Error::PageNotFound => Some(Box::new("The requested page was not found. This usually indicates corruption.")),
            lmdb::Error::Corrupted => None,
            lmdb::Error::Panic => None,
            lmdb::Error::VersionMismatch => None,
            lmdb::Error::Invalid => None,
            lmdb::Error::MapFull => None,
            lmdb::Error::DbsFull => None,
            lmdb::Error::ReadersFull => None,
            lmdb::Error::TlsFull => None,
            lmdb::Error::TxnFull => None,
            lmdb::Error::CursorFull => None,
            lmdb::Error::PageFull => None,
            lmdb::Error::MapResized => None,
            lmdb::Error::Incompatible => None,
            lmdb::Error::BadRslot => Some(Box::new("This usually indicates that the operation can't complete because an incompatible transaction is still open.")),
            lmdb::Error::BadTxn => None,
            lmdb::Error::BadValSize => None,
            lmdb::Error::BadDbi => None,
            lmdb::Error::Other(_) => None,
        }
    }

    fn url<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        None
    }
}

impl From<Error> for capnp::Error {
    fn from(dberr: Error) -> capnp::Error {
        capnp::Error::failed(format!("database error: {}", dberr.to_string()))
    }
}
