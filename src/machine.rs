use std::str::FromStr;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use slog::Logger;

use serde::{Serialize, Deserialize};

use std::sync::Arc;
use smol::lock::RwLock;

use crate::error::Result;
use crate::config::Settings;
use crate::access;

use capnp::Error;

use uuid::Uuid;

use lmdb::{Transaction, RwTransaction, Cursor};

use smol::channel::{Receiver, Sender};

use futures_signals::signal::*;

use crate::registries::StatusSignal;

pub type ID = Uuid;

/// Status of a Machine
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[repr(u8)]
pub enum Status {
    /// Not currently used by anybody
    Free,
    /// Used by somebody
    Occupied,
    /// Not used by anybody but also can not be used. E.g. down for maintenance
    Blocked,
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
    /// Computer-readable identifier for this machine
    // Implicit in database since it's the key.
    #[serde(skip)]
    id: ID,

    /// The human-readable name of the machine. Does not need to be unique
    name: String,

    /// The required permission to use this machine.
    perm: access::PermIdentifier,

    /// The state of the machine as bffh thinks the machine *should* be in.
    ///
    /// This is a Signal generator. Subscribers to this signal will be notified of changes. In the
    /// case of an actor it should then make sure that the real world matches up with the set state
    state: Mutable<Status>,
}

impl Machine {
    pub fn new(id: Uuid, name: String, perm: access::PermIdentifier) -> Machine {
        Machine {
            id: id,
            name: name,
            perm: perm,
            state: Mutable::new(Status::Free),
        }
    }

    /// Generate a signal from the internal state.
    ///
    /// A signal is a lossy stream of state changes. Lossy in that if changes happen in quick
    /// succession intermediary values may be lost. But this isn't really relevant in this case
    /// since the only relevant state is the latest one.
    /// dedupe ensures that if state is changed but only changes to the value it had beforehand
    /// (could for example happen if the machine changes current user but stays activated) no
    /// update is sent.
    pub fn signal(&self) -> StatusSignal {
        Box::pin(self.state.signal().dedupe())
    }

    /// Requests to use a machine. Returns `true` if successful.
    ///
    /// This will update the internal state of the machine, notifying connected actors, if any.
    pub fn request_use<T: Transaction>
        ( &mut self
        , txn: &T
        , pp: &access::PermissionsProvider
        , who: access::UserIdentifier
        ) -> Result<bool>
    {
        if pp.check(txn, who, self.perm)? {
            self.state.set(Status::Occupied);
            return Ok(true);
        } else {
            return Ok(false);
        }
    }

    pub fn set_state(&mut self, state: Status) {
        self.state.set(state)
    }
}

pub struct MachinesProvider {
    log: Logger,
    db: lmdb::Database,
}

impl MachinesProvider {
    pub fn new(log: Logger, db: lmdb::Database) -> Self {
        Self { log, db }
    }

    pub fn get_machine<T: Transaction>(&self, txn: &T, uuid: Uuid) 
        -> Result<Option<Machine>> 
    {
        match txn.get(self.db, &uuid.as_bytes()) {
            Ok(bytes) => {
                let mut machine: Machine = flexbuffers::from_slice(bytes)?;
                machine.id = uuid;

                Ok(Some(machine))
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

    pub fn load_db(&mut self, txn: &mut RwTransaction, mut path: PathBuf) -> Result<()> {
       path.push("machines");
       for entry in std::fs::read_dir(path)? {
           let entry = entry?;
           let path = entry.path();
           if path.is_file() {
               // will only ever be none if the path has no file name and then how is it a file?!
               let machID_str = path
                   .file_stem().expect("Found a file with no filename?")
                   .to_str().expect("Found an OsStr that isn't valid Unicode. Fix your OS!");
               let machID = match uuid::Uuid::from_str(machID_str) {
                   Ok(i) => i,
                   Err(e) => {
                       warn!(self.log, "File {} had a invalid name. Expected an u64 in [0-9a-z] hex with optional file ending: {}. Skipping!", path.display(), e);
                       continue;
                   }
               };
               let s = match fs::read_to_string(path.as_path()) {
                   Ok(s) => s,
                   Err(e) => {
                       warn!(self.log, "Failed to open file {}: {}, skipping!"
                            , path.display()
                            , e);
                       continue;
                   }
               };
               let mach: Machine = match toml::from_str(&s) {
                   Ok(r) => r,
                   Err(e) => {
                       warn!(self.log, "Failed to parse mach at path {}: {}, skipping!"
                            , path.display()
                            , e);
                       continue;
                   }
               };
               self.put_machine(txn, machID, mach)?;
               debug!(self.log, "Loaded machine {}", machID);
           } else {
               warn!(self.log, "Path {} is not a file, skipping!", path.display());
           }
       }

       Ok(())
    }

    pub fn dump_db<T: Transaction>(&self, txn: &T, mut path: PathBuf) -> Result<()> {
        path.push("machines");
       let mut mach_cursor = txn.open_ro_cursor(self.db)?;
       for buf in mach_cursor.iter_start() {
           let (kbuf, vbuf) = buf?;
           let machID = uuid::Uuid::from_slice(kbuf).unwrap();
           let mach: Machine = flexbuffers::from_slice(vbuf)?;
           let filename = format!("{}.yml", machID.to_hyphenated().to_string());
           path.set_file_name(filename);
           let mut fp = std::fs::File::create(&path)?;
           let out = toml::to_vec(&mach)?;
           fp.write_all(&out)?;
       }

       Ok(())
    }
}

pub fn init(log: Logger, config: &Settings, env: &lmdb::Environment) -> Result<MachinesProvider> {
    let mut flags = lmdb::DatabaseFlags::empty();
    flags.set(lmdb::DatabaseFlags::INTEGER_KEY, true);
    let machdb = env.create_db(Some("machines"), flags)?;
    debug!(&log, "Opened machine db successfully.");

    Ok(MachinesProvider::new(log, machdb))
}
