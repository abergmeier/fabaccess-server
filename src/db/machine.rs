use slog::Logger;

use serde::{Serialize, Deserialize};

use std::sync::Arc;

use crate::error::Result;
use crate::config::Config;

use crate::db::user::UserId;

pub mod internal;
use internal::Internal;

pub type MachineIdentifier = String;
pub type Priority = u64;

/// Status of a Machine
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Status {
    /// Not currently used by anybody
    Free,
    /// Used by somebody
    InUse(Option<UserId>),
    /// Was used by somebody and now needs to be checked for cleanliness
    ToCheck(UserId),
    /// Not used by anybody but also can not be used. E.g. down for maintenance
    Blocked(UserId),
    /// Disabled for some other reason
    Disabled,
    /// Reserved
    Reserved(UserId),
}


#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// The status of the machine
pub struct MachineState {
    pub state: Status,
}

impl MachineState {
    pub fn new() -> Self {
        Self { state: Status::Free }
    }

    pub fn free() -> Self {
        Self { state: Status::Free }
    }

    pub fn used(uid: Option<UserId>) -> Self {
        Self { state: Status::InUse(uid) }
    }
}

pub fn init(log: Logger, _config: &Config, env: Arc<lmdb::Environment>) -> Result<Internal> {
    let mut flags = lmdb::DatabaseFlags::empty();
    flags.set(lmdb::DatabaseFlags::INTEGER_KEY, true);
    let machdb = env.create_db(Some("machines"), flags)?;
    debug!(&log, "Opened machine db successfully.");

    Ok(Internal::new(log, env, machdb))
}
