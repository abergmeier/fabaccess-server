// FIXME: No.
#![allow(dead_code)]
#![forbid(unused_imports)]

//mod modules;
//mod log;
//mod config;
//mod connection;
//mod machine;
//mod builtin;
//mod server;
//mod actor;
//mod initiator;
mod space;

mod resource;
mod schema;
mod state;
pub mod db;
mod network;
pub mod oid;
mod varint;
pub mod error;
pub mod config;
mod permissions;


mod runtime {
    use bastion::prelude::*;

    pub fn startup() {
        let config = Config::new().hide_backtraces();

        Bastion::init_with(config);

        Bastion::start();

        let sup = Bastion::supervisor(|sp| {
            sp  .with_strategy(SupervisionStrategy::OneForAll)
                .children(|children| {
                    children
                })
        }).expect("Failed to create supervisor");
    }

    pub fn run() {
        Bastion::block_until_stopped()
    }
}