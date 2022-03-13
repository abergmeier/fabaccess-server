use std::ops::Deref;
use crate::utils::oid::ObjectIdentifier;
use once_cell::sync::Lazy;
use rkyv::{Archive, Archived, Deserialize, Serialize, Infallible};
use rkyv_dyn::{DynError, DynSerializer};
use std::str::FromStr;

use crate::oidvalue;
use crate::resources::state::{State};
use crate::resources::state::value::Value;
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
#[archive_attr(derive(Debug, PartialEq))]
/// The status of the machine
pub struct MachineState {
    pub state: Status,
    pub previous: Option<User>,
}

impl MachineState {
    pub fn new() -> Self {
        Self {
            state: Status::Free,
            previous: None,
        }
    }

    pub fn from(dbstate: &Archived<State>) -> Self {
        use std::any::TypeId;
        let state: &Archived<MachineState> = &dbstate.inner;
        Deserialize::deserialize(state, &mut Infallible).unwrap()
    }
    pub fn to_state(&self) -> State {
        State {
            inner: self.clone()
        }
    }

    pub fn free(previous: Option<User>) -> Self {
        Self {
            state: Status::Free,
            previous,
        }
    }

    pub fn used(user: User, previous: Option<User>) -> Self {
        Self {
            state: Status::InUse(user),
            previous,
        }
    }

    pub fn blocked(user: User, previous: Option<User>) -> Self {
        Self {
            state: Status::Blocked(user),
            previous,
        }
    }

    pub fn disabled(previous: Option<User>) -> Self {
        Self {
            state: Status::Disabled,
            previous,
        }
    }

    pub fn reserved(user: User, previous: Option<User>) -> Self {
        Self {
            state: Status::Reserved(user),
            previous,
        }
    }

    pub fn check(user: User) -> Self {
        Self {
            state: Status::ToCheck(user),
            previous: Some(user),
        }
    }
}

pub static OID_TYPE: Lazy<ObjectIdentifier> =
    Lazy::new(|| ObjectIdentifier::from_str("1.3.6.1.4.1.48398.612.1.14").unwrap());
pub static OID_VALUE: Lazy<ObjectIdentifier> =
    Lazy::new(|| ObjectIdentifier::from_str("1.3.6.1.4.1.48398.612.2.4").unwrap());
oidvalue!(OID_TYPE, MachineState, ArchivedMachineState);
