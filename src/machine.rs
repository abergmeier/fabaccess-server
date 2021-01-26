use std::ops::Deref;
use std::iter::FromIterator;
use std::sync::Arc;
use futures_util::lock::Mutex;
use std::path::Path;
use std::task::{Poll, Context};
use std::pin::Pin;
use std::future::Future;

use std::collections::HashMap;
use std::fs;

use serde::{Serialize, Deserialize};

use futures::future::BoxFuture;

use futures_signals::signal::Signal;
use futures_signals::signal::SignalExt;
use futures_signals::signal::{Mutable, ReadOnlyMutable};

use crate::error::{Result, Error};

use crate::db::access;
use crate::db::machine::{MachineIdentifier, MachineState};
use crate::db::user::{User, UserData};

use crate::network::MachineMap;

#[derive(Debug, Clone)]
pub struct Index {
    inner: HashMap<String, Machine>,
}

impl Index {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    pub fn insert(&mut self, key: String, value: Machine) -> Option<Machine> {
        self.inner.insert(key, value)
    }

    pub fn get(&mut self, key: &String) -> Option<Machine> {
        self.inner.get(key).map(|m| m.clone())
    }
}

#[derive(Debug, Clone)]
pub struct Machine {
    inner: Arc<Mutex<Inner>>,
    access: Arc<access::AccessControl>,
}

impl Machine {
    pub fn new(inner: Inner, access: Arc<access::AccessControl>) -> Self {
        Self { inner: Arc::new(Mutex::new(inner)), access: access }
    }

    pub fn construct
        ( id: MachineIdentifier
        , desc: MachineDescription
        , state: MachineState
        , access: Arc<access::AccessControl>
        ) -> Machine
    {
        Self::new(Inner::new(id, desc, state), access)
    }

    pub fn from_file<P: AsRef<Path>>(path: P, access: Arc<access::AccessControl>) 
        -> Result<Vec<Machine>> 
    {
        let mut map: HashMap<MachineIdentifier, MachineDescription> = MachineDescription::load_file(path)?;
        Ok(map.drain().map(|(id, desc)| {
            Self::construct(id, desc, MachineState::new(), access.clone())
        }).collect())
    }

    /// Requests to use a machine. Returns a return token if successful.
    ///
    /// This will update the internal state of the machine, notifying connected actors, if any.
    /// The return token is a channel that considers the machine 'returned' if anything is sent
    /// along it or if the sending end gets dropped. Anybody who holds this token needs to check if
    /// the receiving end was canceled which indicates that the machine has been taken off their
    /// hands.
    pub fn request_state_change(&self, who: Option<&User>, new_state: MachineState) 
        -> BoxFuture<'static, Result<ReturnToken>>
    {
        let this = self.clone();
        let udata: Option<UserData> = who.map(|u| u.data.clone());

        let f = async move {
            if let Some(udata) = udata {
                let mut guard = this.inner.try_lock().unwrap();
                if this.access.check(&udata, &guard.desc.privs.write).await? {
                    return guard.do_state_change(new_state);
                }
            } else {
                if new_state == MachineState::free() {
                    let mut guard = this.inner.try_lock().unwrap();
                    return guard.do_state_change(new_state);
                }
            }

            return Err(Error::Denied);
        };

        Box::pin(f)
    }

    pub fn signal(&self) -> impl Signal<Item=MachineState> {
        let guard = self.inner.try_lock().unwrap();
        guard.signal()
    }
}

impl Deref for Machine {
    type Target = Mutex<Inner>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[derive(Debug)]
/// Internal machine representation
///
/// A machine connects an event from a sensor to an actor activating/deactivating a real-world
/// machine, checking that the user who wants the machine (de)activated has the required
/// permissions.
pub struct Inner {
    /// Globally unique machine readable identifier
    pub id: MachineIdentifier,

    /// Descriptor of the machine
    pub desc: MachineDescription,

    /// The state of the machine as bffh thinks the machine *should* be in.
    ///
    /// This is a Signal generator. Subscribers to this signal will be notified of changes. In the
    /// case of an actor it should then make sure that the real world matches up with the set state
    state: Mutable<MachineState>,
    reset: Option<MachineState>,
    rx: Option<futures::channel::oneshot::Receiver<()>>,
}

impl Inner {
    pub fn new ( id: MachineIdentifier
               , desc: MachineDescription
               , state: MachineState
               ) -> Inner 
    {
        Inner {
            id: id,
            desc: desc,
            state: Mutable::new(state),
            reset: None,
            rx: None,
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

    pub fn do_state_change(&mut self, new_state: MachineState) -> Result<ReturnToken> {
            let (tx, rx) = futures::channel::oneshot::channel();
            let old_state = self.state.replace(new_state);
            self.reset.replace(old_state);
            // Also this drops the old receiver, which will signal to the initiator that the
            // machine has been taken off their hands.
            self.rx.replace(rx);
            return Ok(tx);
    }

    pub fn read_state(&self) -> ReadOnlyMutable<MachineState> {
        self.state.read_only()
    }

    pub fn get_signal(&self) -> impl Signal {
        self.state.signal_cloned()
    }

    pub fn reset_state(&mut self) {
        if let Some(state) = self.reset.take() {
            self.state.replace(state);
        }
    }
}

pub type ReturnToken = futures::channel::oneshot::Sender<()>;

impl Future for Inner {
    type Output = MachineState;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let mut this = &mut *self;
        // TODO Return this on exit
        if false {
            return Poll::Ready(self.state.get_cloned());
        }

        // Check if the return token was sent/dropped
        if let Some(mut rx) = this.rx.take() {
            match Future::poll(Pin::new(&mut rx), cx) {
                // Regardless if we were canceled or properly returned, reset.
                Poll::Ready(_) => self.reset_state(),
                Poll::Pending => { this.rx.replace(rx); },
            }
        }

        Poll::Pending
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
    pub privs: access::PrivilegesBuf,
}

impl MachineDescription {
    pub fn load_file<P: AsRef<Path>>(path: P) -> Result<HashMap<MachineIdentifier, MachineDescription>> {
        let content = fs::read(path)?;
        Ok(toml::from_slice(&content[..])?)
    }
}

pub fn load(config: &crate::config::Config, access: Arc<access::AccessControl>) 
    -> Result<MachineMap> 
{
    let mut map = config.machines.clone();

    let it = map.drain()
        .map(|(k,v)| {
            // TODO: Read state from the state db
            (v.name.clone(), Machine::construct(k, v, MachineState::new(), access.clone()))
        });
    Ok(HashMap::from_iter(it))
}

#[cfg(test_DISABLED)]
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
