use crate::aggregate::Id;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub(crate) struct Attributes {
    attributes: HashMap<FieldKey, console_api::Attribute>,
}

#[derive(Debug, Clone)]
pub(crate) struct Update {
    pub(crate) field: console_api::Field,
    pub(crate) op: Option<UpdateOp>,
    pub(crate) unit: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) enum UpdateOp {
    Add,
    Override,
    Sub,
}

/// Represents a key for a `proto::field::Name`. Because the
/// proto::field::Name might not be unique we also include the
/// resource id in this key
#[derive(Debug, Hash, PartialEq, Eq)]
struct FieldKey {
    update_id: Id,
    field_name: console_api::field::Name,
}
