use slog::Logger;

use std::sync::Arc;
use smol::lock::RwLock;

use std::pin::Pin;
use futures::ready;
use futures::prelude::*;
use futures::channel::mpsc;
use futures::task::{Context, Poll, Spawn};
use futures_signals::signal::Signal;

use crate::db::machine::MachineState;

use std::collections::HashMap;

pub trait Actuator {
    fn apply(&mut self, state: MachineState);
}

pub struct Dummy;

impl Actuator for Dummy {
    fn apply(&mut self, state: MachineState) {
        println!("New state for dummy actuator: {:?}", state);
    }
}
