use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};

use slog::Logger;

use serde::{Serialize, Deserialize};

use std::sync::Arc;
use smol::lock::RwLock;

use crate::error::Result;
use crate::config::Settings;

use capnp::Error;

use uuid::Uuid;

use lmdb::{Transaction, RwTransaction};

use smol::channel::{Receiver, Sender};

use futures_signals::signal::*;

/// Status of a Machine
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Status {
    /// Not currently used by anybody
    Free,
    /// Used by somebody
    Occupied,
    /// Not used by anybody but also can not be used. E.g. down for maintenance
    Blocked,
}

pub struct MachinesProvider {
    log: Logger,
    mdb: MachineDB,
}

impl MachinesProvider {
    pub fn new(log: Logger, mdb: MachineDB) -> Self {
        Self { log, mdb }
    }
}

#[derive(Clone)]
pub struct Machines {
    inner: Arc<RwLock<MachinesProvider>>,
}
impl Machines {
    pub fn new(inner: Arc<RwLock<MachinesProvider>>) -> Self {
        Self { inner }
    }
}

#[derive(Clone)]
pub struct GiveBack {
    mdb: Arc<RwLock<MachinesProvider>>,
    uuid: Uuid,
}
impl GiveBack {
    pub fn new(mdb: Arc<RwLock<MachinesProvider>>, uuid: Uuid) -> Self {
        Self { mdb, uuid }
    }
}

fn uuid_from_api(uuid: crate::api::api_capnp::u_u_i_d::Reader) -> Uuid {
    let uuid0 = uuid.get_uuid0() as u128;
    let uuid1 = uuid.get_uuid1() as u128;
    let num: u128 = (uuid1 << 64) + uuid0;
    Uuid::from_u128(num)
}
fn api_from_uuid(uuid: Uuid, mut wr: crate::api::api_capnp::u_u_i_d::Builder) {
    let num = uuid.to_u128_le();
    let uuid0 = num as u64;
    let uuid1 = (num >> 64) as u64;
    wr.set_uuid0(uuid0);
    wr.set_uuid1(uuid1);
}

#[derive(Clone)]
pub struct MachineManager {
    mdb: Arc<RwLock<MachinesProvider>>,
    uuid: Uuid,
}

impl MachineManager {
    pub fn new(uuid: Uuid, mdb: Arc<RwLock<MachinesProvider>>) -> Self {
        Self { mdb, uuid }
    }
}

#[derive(Debug, Serialize, Deserialize)]
/// Internal machine representation
///
/// A machine connects an event from a sensor to an actor activating/deactivating a real-world
/// machine, checking that the user who wants the machine (de)activated has the required
/// permissions.
pub struct Machine {
    /// The human-readable name of the machine. Does not need to be unique
    name: String,

    /// The required permission to use this machine.
    perm: String,

    /// The state of the machine as bffh thinks the machine *should* be in.
    ///
    /// This is a Signal generator. Subscribers to this signal will be notified of changes. In the
    /// case of an actor it should then make sure that the real world matches up with the set state
    state: Mutable<Status>,
}

impl Machine {
    pub fn new(name: String, perm: String) -> Machine {
        Machine {
            name: name,
            perm: perm,
            state: Mutable::new(Status::Free),
        }
    }

    pub fn signal(&self) -> MutableSignal<Status> {
        self.state.signal()
    }
}

pub struct MachineDB {
    db: lmdb::Database,
}

impl MachineDB {
    pub fn new(db: lmdb::Database) -> Self {
        Self { db }
    }

    pub fn get_machine<T: Transaction>(&self, txn: &T, uuid: Uuid) 
        -> Result<Option<Machine>> 
    {
        match txn.get(self.db, &uuid.as_bytes()) {
            Ok(bytes) => {
                Ok(Some(flexbuffers::from_slice(bytes)?))
            },
            Err(lmdb::Error::NotFound) => { Ok(None) },
            Err(e) => { Err(e.into()) },
        }
    }

    pub fn put_machine( &self, txn: &mut RwTransaction, uuid: Uuid, machine: Machine) 
        -> Result<()>
    {
        let bytes = flexbuffers::to_vec(machine)?;
        txn.put(self.db, &uuid.as_bytes(), &bytes, lmdb::WriteFlags::empty())?;

        Ok(())
    }
}

pub async fn init(log: Logger, config: &Settings) -> Result<MachinesProvider> {
    unimplemented!()
}
