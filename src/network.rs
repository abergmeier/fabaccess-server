use std::fmt;

use std::sync::{Arc, Mutex, MutexGuard, TryLockResult};
use std::collections::HashMap;

use smol::Executor;

use futures::channel::mpsc;
use futures_signals::signal::{Signal, MutableSignalCloned, Mutable};

use crate::machine::Machine;
use crate::actor::{Actor, ActorSignal};
use crate::initiator::Initiator;
use crate::db::machine::MachineState;

use crate::error::Result;

pub type MachineMap = HashMap<String, Machine>;
pub type ActorMap = HashMap<String, Mutex<mpsc::Sender<Option<ActorSignal>>>>;
pub type InitMap = HashMap<String, Mutable<Option<Machine>>>;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    NoSuchInitiator,
    NoSuchMachine,
    NoSuchActor,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::NoSuchInitiator => write!(f, "No initiator found with that name"),
            Error::NoSuchActor => write!(f, "No actor found with that name"),
            Error::NoSuchMachine => write!(f, "No machine found with that name"),
        }
    }
}

/// Main signal network
///
/// Network as per FRP, not the one with packages and frames
// TODO De/Serialize established connection on startup/shutdown.
pub struct Network {
    inits: InitMap,

    // Store connections 
    //miconn: Vec<(String, String)>,

    pub machines: MachineMap,

    // Store connections 
    //maconn: Vec<(String, String)>,

    actors: ActorMap,
}

impl Network {
    pub fn new(machines: MachineMap, actors: ActorMap, inits: InitMap) -> Self {
        Self { machines, actors, inits }
    }

    pub fn connect_init(&self, init_key: &String, machine_key: &String) -> Result<()> {
        let init = self.inits.get(init_key)
            .ok_or(Error::NoSuchInitiator)?;
        let machine = self.machines.get(machine_key)
            .ok_or(Error::NoSuchMachine)?;

        init.set(Some(machine.clone()));
        Ok(())
    }

    pub fn connect_actor(&mut self, machine_key: &String, actor_key: &String)
        -> Result<()>
    {
        let machine = self.machines.get(machine_key)
            .ok_or(Error::NoSuchMachine)?;
        let actor = self.actors.get(actor_key)
            .ok_or(Error::NoSuchActor)?;

        // FIXME Yeah this should not unwrap. Really, really shoudln't.
        let mut guard = actor.try_lock().unwrap();

        guard.try_send(Some(Box::new(machine.signal()))).map_err(|_| Error::NoSuchActor.into())
    }
}
