use rkyv::{Archive, Serialize, Deserialize};









#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Archive, Serialize, Deserialize)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Resource {
    uuid: u128,
    id: String,
    name_idx: u64,
    description_idx: u64,
}