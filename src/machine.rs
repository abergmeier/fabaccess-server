use std::path::Path;
use std::collections::HashMap;
use std::fs;

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
    /// Globally unique machine readable identifier
    pub id: MachineIdentifier,

    /// Descriptor of the machine
    pub desc: MachineDescription,

    /// The state of the machine as bffh thinks the machine *should* be in.
    ///
    /// This is a Signal generator. Subscribers to this signal will be notified of changes. In the
    /// case of an actor it should then make sure that the real world matches up with the set state
    state: Mutable<MachineState>,
}

impl Machine {
    pub fn new(id: MachineIdentifier, desc: MachineDescription, perm: access::PermIdentifier) -> Machine {
        Machine {
            id: id,
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
    pub async fn request_use
        ( &mut self
        , access: access::AccessControl
        , who: &User
        ) -> Result<bool>
    {
        // TODO: Check different levels
        if access.check(who, &self.desc.privs.write).await? {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// A description of a machine
///
/// This is the struct that a machine is serialized to/from.
/// Combining this with the actual state of the system will return a machine
pub struct MachineDescription {
    /// The name of the machine. Doesn't need to be unique but is what humans will be presented.
    pub name: String,
    /// An optional description of the Machine.
    pub description: Option<String>,

    /// The permission required
    #[serde(flatten)]
    privs: access::PrivilegesBuf,
}

impl MachineDescription {
    pub fn load_file<P: AsRef<Path>>(path: P) -> Result<HashMap<MachineIdentifier, MachineDescription>> {
        let content = fs::read(path)?;
        Ok(toml::from_slice(&content[..])?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::iter::FromIterator;

    use crate::db::access::{PermissionBuf, PrivilegesBuf};

    #[test]
    fn load_examples_descriptions_test() {
        let mut machines = MachineDescription::load_file("examples/machines.toml")
            .expect("Couldn't load the example machine defs. Does `examples/machines.toml` exist?");

        let expected = 
            vec![
            (Uuid::parse_str("e5408099-d3e5-440b-a92b-3aabf7683d6b").unwrap(),
            MachineDescription {
                name: "Somemachine".to_string(),
                description: None,
                privs: PrivilegesBuf {
                    disclose: PermissionBuf::from_string("lab.some.disclose".to_string()),
                    read: PermissionBuf::from_string("lab.some.read".to_string()),
                    write: PermissionBuf::from_string("lab.some.write".to_string()),
                    manage: PermissionBuf::from_string("lab.some.admin".to_string()),
                },
            }),
            (Uuid::parse_str("eaabebae-34d1-4a3a-912a-967b495d3d6e").unwrap(),
            MachineDescription {
                name: "Testmachine".to_string(),
                description: Some("An optional description".to_string()),
                privs: PrivilegesBuf {
                    disclose: PermissionBuf::from_string("lab.test.read".to_string()),
                    read: PermissionBuf::from_string("lab.test.read".to_string()),
                    write: PermissionBuf::from_string("lab.test.write".to_string()),
                    manage: PermissionBuf::from_string("lab.test.admin".to_string()),
                },
            }),
            ];

        for (id, machine) in expected.into_iter() {

            assert_eq!(machines.remove(&id).unwrap(), machine);
        }

        assert!(machines.is_empty());
    }
}
