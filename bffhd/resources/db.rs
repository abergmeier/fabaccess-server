use rkyv::{Archive, Serialize, Deserialize};

use crate::db::DB;
use crate::db::{AlignedAdapter, AllocAdapter};
use crate::db::RawDB;
use std::sync::Arc;
use crate::db::{Environment, DatabaseFlags};
use crate::db::Result;
use crate::resources::state::db::StateDB;

#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Archive, Serialize, Deserialize)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Resource {
    uuid: u128,
    id: String,
    name_idx: u64,
    description_idx: u64,
}