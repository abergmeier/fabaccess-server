use crate::utils::oid::ObjectIdentifier;
use once_cell::sync::Lazy;
use rkyv::{Archive, Deserialize, Serialize};
use rkyv_dyn::{DynError, DynSerializer};
use std::str::FromStr;

use crate::oidvalue;
use crate::session::SessionHandle;
use crate::users::User;

/// Status of a Machine
#[derive(
    Copy,
    Clone,
    PartialEq,
    Eq,
    Debug,
    Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
)]
#[archive_attr(derive(Debug, PartialEq, serde::Serialize, serde::Deserialize))]
pub enum Status {
    /// Not currently used by anybody
    Free,
    /// Used by somebody
    InUse(User),
    /// Was used by somebody and now needs to be checked for cleanliness
    ToCheck(User),
    /// Not used by anybody but also can not be used. E.g. down for maintenance
    Blocked(User),
    /// Disabled for some other reason
    Disabled,
    /// Reserved
    Reserved(User),
}

#[derive(
    Copy,
    Clone,
    PartialEq,
    Eq,
    Debug,
    Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
)]
#[archive_attr(derive(Debug, PartialEq, serde::Serialize, serde::Deserialize))]
/// The status of the machine
pub struct MachineState {
    pub state: Status,
}

impl MachineState {
    pub fn new() -> Self {
        Self {
            state: Status::Free,
        }
    }

    pub fn from(state: Status) -> Self {
        Self { state }
    }

    pub fn free() -> Self {
        Self {
            state: Status::Free,
        }
    }

    pub fn used(user: User) -> Self {
        Self {
            state: Status::InUse(user),
        }
    }

    pub fn blocked(user: User) -> Self {
        Self {
            state: Status::Blocked(user),
        }
    }

    pub fn disabled() -> Self {
        Self {
            state: Status::Disabled,
        }
    }

    pub fn reserved(user: User) -> Self {
        Self {
            state: Status::Reserved(user),
        }
    }

    pub fn check(user: User) -> Self {
        Self {
            state: Status::ToCheck(user),
        }
    }

    pub fn make_used(&mut self, session: SessionHandle) -> Self {
        unimplemented!()
    }
}

static OID_TYPE: Lazy<ObjectIdentifier> =
    Lazy::new(|| ObjectIdentifier::from_str("1.3.6.1.4.1.48398.612.1.14").unwrap());
static OID_VALUE: Lazy<ObjectIdentifier> =
    Lazy::new(|| ObjectIdentifier::from_str("1.3.6.1.4.1.48398.612.2.4").unwrap());
oidvalue!(OID_TYPE, MachineState, ArchivedMachineState);
