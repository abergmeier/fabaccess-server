use serde::{Serialize, Deserialize};

use futures_signals::signal::Signal;
use futures_signals::signal::SignalExt;
use futures_signals::signal::Mutable;

use uuid::Uuid;

use crate::error::Result;

use crate::db::user::User;
use crate::db::access;
use crate::db::machine::{MachineIdentifier, Status, MachineState};

#[derive(Debug)]
/// Internal machine representation
///
/// A machine connects an event from a sensor to an actor activating/deactivating a real-world
/// machine, checking that the user who wants the machine (de)activated has the required
/// permissions.
pub struct Machine {
    /// Descriptor of the machine
    desc: MachineDescription,

    /// The state of the machine as bffh thinks the machine *should* be in.
    ///
    /// This is a Signal generator. Subscribers to this signal will be notified of changes. In the
    /// case of an actor it should then make sure that the real world matches up with the set state
    state: Mutable<MachineState>,
}

impl Machine {
    pub fn new(desc: MachineDescription, perm: access::PermIdentifier) -> Machine {
        Machine {
            desc: desc,
            state: Mutable::new(MachineState { state: Status::Free}),
        }
    }

    /// Generate a signal from the internal state.
    ///
    /// A signal is a lossy stream of state changes. Lossy in that if changes happen in quick
    /// succession intermediary values may be lost. But this isn't really relevant in this case
    /// since the only relevant state is the latest one.
    pub fn signal(&self) -> impl Signal<Item=MachineState> {
        // dedupe ensures that if state is changed but only changes to the value it had beforehand
        // (could for example happen if the machine changes current user but stays activated) no
        // update is sent.
        Box::pin(self.state.signal_cloned().dedupe_cloned())
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
        // TODO: Check different levels
        if pp.check(who, &self.desc.privs.write)? {
            self.state.set(MachineState { state: Status::InUse(who.id.clone()) });
            return Ok(true);
        } else {
            return Ok(false);
        }
    }

    pub fn set_state(&mut self, state: Status) {
        self.state.set(MachineState { state })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// A description of a machine
///
/// This is the struct that a machine is serialized to/from.
/// Combining this with the actual state of the system will return a machine
pub struct MachineDescription {
    /// The main machine identifier. This must be unique.
    id: MachineIdentifier,
    /// The name of the machine. Doesn't need to be unique but is what humans will be presented.
    name: String,
    /// An optional description of the Machine.
    description: Option<String>,

    /// The permission required
    privs: access::PrivilegesBuf,
}
