use futures_signals::signal::Signal;
use futures_signals::signal::SignalExt;
use futures_signals::signal::Mutable;

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
    /// Computer-readable identifier for this machine
    id: MachineIdentifier,

    /// The human-readable name of the machine. Does not need to be unique
    name: String,

    /// The required permissions to use this machine.
    perm: access::PermIdentifier,

    /// The state of the machine as bffh thinks the machine *should* be in.
    ///
    /// This is a Signal generator. Subscribers to this signal will be notified of changes. In the
    /// case of an actor it should then make sure that the real world matches up with the set state
    state: Mutable<MachineState>,
}

impl Machine {
    pub fn new(id: Uuid, name: String, perm: access::PermIdentifier) -> Machine {
        Machine {
            id: id,
            name: name,
            perm: perm,
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
        if pp.check(who, &self.perm)? {
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
