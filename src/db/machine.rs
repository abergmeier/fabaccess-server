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

use futures::{Future, Stream, StreamExt};
use futures_signals::signal::*;

use crate::registries::StatusSignal;
use crate::db::user::User;

mod internal;
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// The status of the machine
pub struct MachineState {
    state: Status,
}

// TODO split up for non-writable Definition Databases
pub trait MachineDB {
    fn get_status(&self, machID: &MachineIdentifier) 
        -> impl Future<Output=Result<Option<MachineState>>>;
    fn put_status(&self, machID: &MachineIdentifier, machine: MachineState) 
        -> impl Future<Output=Result<()>>;

    fn iter_status(&self) -> impl Stream<Output=Result<MachineState>>;
}

pub fn init(log: Logger, config: &Settings, env: Arc<lmdb::Environment>) -> Result<Internal> {
    let mut flags = lmdb::DatabaseFlags::empty();
    flags.set(lmdb::DatabaseFlags::INTEGER_KEY, true);
    let machdb = env.create_db(Some("machines"), flags)?;
    debug!(&log, "Opened machine db successfully.");

    Ok(Internal::new(log, env, machdb))
}
