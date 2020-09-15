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

/// Status of a Machine
#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
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

    pub fn use_(&mut self, uuid: &Uuid) -> std::result::Result<(), capnp::Error> {
        if let Some(m) = self.mdb.get_mut(uuid) {
            match m.status {
                Status::Free => {
                    trace!(self.log, "Granted use on machine {}", uuid);

                    m.status = Status::Occupied;

                    Ok(())
                },
                Status::Occupied => {
                    info!(self.log, "Attempted use on an occupied machine {}", uuid);
                    Err(Error::failed("Machine is occupied".to_string()))
                },
                Status::Blocked => {
                    info!(self.log, "Attempted use on a blocked machine {}", uuid);
                    Err(Error::failed("Machine is blocked".to_string()))
                }
            }
        } else {
            info!(self.log, "Attempted use on invalid machine {}", uuid);
            Err(Error::failed("No such machine".to_string()))
        }
    }

    pub fn give_back(&mut self, uuid: &Uuid) -> std::result::Result<(), capnp::Error> {
        if let Some(m) = self.mdb.get_mut(uuid) {
            m.status = Status::Free;
        } else {
            warn!(self.log, "A giveback was issued for a unknown machine {}", uuid);
        }

        Ok(())
    }

    pub fn get_perm_req(&self, uuid: &Uuid) -> Option<String> {
        self.mdb.get(uuid).map(|m| m.perm.clone())
    }

    pub fn set_blocked(&mut self, uuid: &Uuid, blocked: bool) -> std::result::Result<(), capnp::Error> {
        // If the value can not be found map doesn't run and ok_or changes it into a Err with the
        // given error value
        self.mdb.get_mut(uuid).map(|m| m.set_blocked(blocked))
            .ok_or(capnp::Error::failed("No such machine".to_string()))
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

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Machine {
    pub name: String,
    pub location: String,
    pub status: Status,
    pub perm: String,
}

impl Machine {
    pub fn new(name: String, location: String, perm: String) -> Machine {
        Machine {
            name: name,
            location: location,
            status: Status::Free,
            perm: perm,
        }
    }

    pub fn set_blocked(&mut self, blocked: bool) {
        if blocked {
            self.status = Status::Blocked;
        } else {
            self.status = Status::Free;
        }
    }
}

struct MachineDB {
    db: lmdb::Database,
}

impl MachineDB {
    pub fn new(db: lmdb::Database) -> Self {
        Self { db }
    }

    pub fn get_machine<T: Transaction>(&self, txn: &T, machine_id: MachineIdentifier) 
        -> Result<Option<Machine>> 
    {
        match txn.get(self.db, &machine_id.to_ne_bytes()) {
            Ok(bytes) => {
                Ok(Some(flexbuffers::from_slice(bytes)?))
            },
            Err(lmdb::Error::NotFound) => { Ok(None) },
            Err(e) => { Err(e.into()) },
        }
    }
}

pub async fn init(log: Logger, config: &Settings) -> Result<MachinesProvider> {
    unimplemented!()
}
