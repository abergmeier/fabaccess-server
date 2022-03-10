use std::sync::Arc;
use async_channel::Sender;
use lmdb::Environment;
use crate::resource::Update;

#[derive(Clone, Debug)]
/// Database of currently valid claims, interests and notify, as far as applicable
pub struct ClaimDB {
    env: Arc<Environment>,
}

pub type UserID = String;
pub type ResourceID = String;
pub struct ClaimEntry {
    subject: UserID,
    target: ResourceID,
    level: Level,
}

enum Level {
    Claim(Claim),
    Interest(Interest),
    Notify(Notify),
}

#[derive(Debug)]
/// A claim on a resource grants permission to update state
///
/// This permission is not necessarily exclusive, depending on the resource in question.
pub struct Claim {
    /// Sending end that can be used to send state updates to a resource.
    pub tx: Sender<Update>,
}

#[derive(Debug)]
/// An interest on a resource indicates that an user wants a resource to be in a specific state
pub struct Interest {

}

#[derive(Debug)]
/// A notify indicates that an user wants to be informed about changes in a resources' state
pub struct Notify {

}