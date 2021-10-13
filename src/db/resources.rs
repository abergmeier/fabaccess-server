use rkyv::{
    Archive,
    Serialize,
    Deserialize,
};

use super::{
    AllocAdapter,
    DB,
};

#[derive(Archive, Serialize, Deserialize)]
pub struct Resource {
    uuid: u128,
    id: String,
    name_idx: u64,
    description_idx: u64,
}

pub struct ResourceDB {
    db: DB<AllocAdapter<Resource>>,
}
