use std::sync::Arc;
use async_channel::Sender;
use lmdb::Environment;
use crate::resources::driver::Update;

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
/// A claim on a resources grants permission to update state
///
/// This permission is not necessarily exclusive, depending on the resources in question.
pub struct Claim {
    /// Sending end that can be used to send state updates to a resources.
    pub tx: Sender<Update>,
}

#[derive(Debug)]
/// An interest on a resources indicates that an user wants a resources to be in a specific state
pub struct Interest {

}

#[derive(Debug)]
/// A notify indicates that an user wants to be informed about changes in a resources' state
pub struct Notify {

}