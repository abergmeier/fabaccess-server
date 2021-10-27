#![forbid(unused_imports)]

pub mod config;
pub mod db;
pub mod error;
pub mod network;
pub mod oid;
pub mod permissions;
pub mod resource;
pub mod schema;
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