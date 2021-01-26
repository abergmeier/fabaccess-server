use slog::Logger;

use serde::{Serialize, Deserialize};

use std::sync::Arc;

use crate::error::Result;
use crate::config::Config;

use uuid::Uuid;

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

pub fn init(log: Logger, config: &Config, env: Arc<lmdb::Environment>) -> Result<Internal> {
    let mut flags = lmdb::DatabaseFlags::empty();
    flags.set(lmdb::DatabaseFlags::INTEGER_KEY, true);
    let machdb = env.create_db(Some("machines"), flags)?;
    debug!(&log, "Opened machine db successfully.");

    Ok(Internal::new(log, env, machdb))
}
