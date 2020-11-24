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
use crate::db::access;

use capnp::Error;

use uuid::Uuid;

use lmdb::{Transaction, RwTransaction, Cursor};

use smol::channel::{Receiver, Sender};

use futures::{Future, Stream, StreamExt};
use futures_signals::signal::*;

use crate::registries::StatusSignal;

use crate::machine::MachineDescription;

pub mod internal;
use internal::Internal;

pub type MachineIdentifier = Uuid;

/// Status of a Machine
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Status {
    /// Not currently used by anybody
    Free,
    /// Used by somebody
    InUse(UserIdentifier),
    /// Was used by somebody and now needs to be checked for cleanliness
    ToCheck(UserIdentifier),
    /// Not used by anybody but also can not be used. E.g. down for maintenance
    Blocked(UserIdentifier),
    /// Disabled for some other reason
    Disabled,
    /// Reserved
    Reserved(UserIdentifier),
}

pub fn uuid_from_api(uuid: crate::schema::api_capnp::u_u_i_d::Reader) -> Uuid {
    let uuid0 = uuid.get_uuid0() as u128;
    let uuid1 = uuid.get_uuid1() as u128;
    let num: u128 = (uuid1 << 64) + uuid0;
    Uuid::from_u128(num)
}
pub fn api_from_uuid(uuid: Uuid, mut wr: crate::schema::api_capnp::u_u_i_d::Builder) {
    let num = uuid.as_u128();
    let uuid0 = num as u64;
    let uuid1 = (num >> 64) as u64;
    wr.set_uuid0(uuid0);
    wr.set_uuid1(uuid1);
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// The status of the machine
pub struct MachineState {
    pub state: Status,
}

pub fn init(log: Logger, config: &Settings, env: Arc<lmdb::Environment>) -> Result<Internal> {
    let mut machine_descriptions = MachineDescription::load_file(&config.machines)?;
    let mut flags = lmdb::DatabaseFlags::empty();
    flags.set(lmdb::DatabaseFlags::INTEGER_KEY, true);
    let machdb = env.create_db(Some("machines"), flags)?;
    debug!(&log, "Opened machine db successfully.");

    Ok(Internal::new(log, env, machdb))
}

type MachMap = HashMap<MachineIdentifier, MachineDescription>;

#[derive(Debug)]
pub struct MachineDB {
    state_db: Internal,
    def_db: MachMap,
    signals_db: HashMap<MachineIdentifier, Mutable<MachineState>>,
}

impl MachineDB {
    pub fn new(state_db: Internal, def_db: MachMap) -> Self {
        Self {
            state_db: state_db,
            def_db: def_db,
            signals_db: HashMap::new(),
        }
    }

    pub fn exists(&self, id: MachineIdentifier) -> bool {
        self.def_db.get(&id).is_some()
    }

    pub fn get_desc(&self, id: &MachineIdentifier) -> Option<&MachineDescription> {
        self.def_db.get(&id)
    }

    pub fn get_state(&self, id: &MachineIdentifier) -> Option<MachineState> {
        // TODO: Error Handling
        self.state_db.get(id).unwrap_or(None)
    }

    pub fn update_state(&self, id: &MachineIdentifier, new_state: MachineState) -> Result<()> {
        // If an error happens the new state was not applied so this will not desync the sources
        self.state_db.put(id, &new_state)?;
        self.signals_db.get(id).map(|mutable| mutable.set(new_state));

        Ok(())
    }

    pub fn get_signal(&self, id: &MachineIdentifier) -> Option<MutableSignalCloned<MachineState>> {
        self.signals_db.get(&id).map(|mutable| mutable.signal_cloned())
    }
}
