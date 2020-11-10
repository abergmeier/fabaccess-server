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

use crate::db::user::UserIdentifier;

use capnp::Error;

use uuid::Uuid;

use lmdb::{Transaction, RwTransaction, Cursor};

use smol::channel::{Receiver, Sender};

use futures_signals::signal::*;

use crate::registries::StatusSignal;
use crate::db::user::User;

mod internal;
use internal::Internal;

pub type MachineIdentifier = Uuid;

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
pub struct GiveBack {
    mdb: Arc<Box<dyn MachineDB>>,
    uuid: Uuid,
}
impl GiveBack {
    pub fn new(mdb: Arc<Box<dyn MachineDB>>, uuid: Uuid) -> Self {
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
    id: MachineIdentifier,

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
    pub fn signal(&self) -> StatusSignal {
        // dedupe ensures that if state is changed but only changes to the value it had beforehand
        // (could for example happen if the machine changes current user but stays activated) no
        // update is sent.
        Box::pin(self.state.signal().dedupe())
    }

    /// Requests to use a machine. Returns `true` if successful.
    ///
    /// This will update the internal state of the machine, notifying connected actors, if any.
    pub fn request_use<P: access::RoleDB>
        ( &mut self
        , pp: &P
        , who: &User
        ) -> Result<bool>
    {
        if pp.check(who, &self.perm)? {
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

// TODO split up for non-writable Definition Databases
pub trait MachineDB {
    fn get_machine(&self, machID: &MachineIdentifier) -> Result<Option<Machine>>;
    fn put_machine(&self, machID: &MachineIdentifier, machine: Machine) -> Result<()>;
}

pub fn init(log: Logger, config: &Settings, env: Arc<lmdb::Environment>) -> Result<Internal> {
    let mut flags = lmdb::DatabaseFlags::empty();
    flags.set(lmdb::DatabaseFlags::INTEGER_KEY, true);
    let machdb = env.create_db(Some("machines"), flags)?;
    debug!(&log, "Opened machine db successfully.");

    Ok(Internal::new(log, env, machdb))
}
