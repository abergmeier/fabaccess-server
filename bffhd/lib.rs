#![forbid(unused_imports)]
#![warn(missing_docs, missing_debug_implementations)]


pub mod db;
pub mod error;
pub mod network;
pub mod oid;
pub mod permissions;
pub mod resource;
pub mod state;
pub mod varint;

use intmap::IntMap;
use resource::ResourceDriver;

#[derive(Debug)]
struct InitiatorDriver;
#[derive(Debug)]
struct ActorDriver;

#[derive(Debug)]
struct System {
    resources: IntMap<ResourceDriver>,
    initiators: IntMap<InitiatorDriver>,
    actors: IntMap<ActorDriver>,
}

#[derive(Debug)]
struct Accountant {

}