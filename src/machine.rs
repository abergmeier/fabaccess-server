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

use futures::Stream;
use futures::future::BoxFuture;
use futures::channel::{mpsc, oneshot};

use futures_signals::signal::Signal;
use futures_signals::signal::SignalExt;
use futures_signals::signal::{Mutable, ReadOnlyMutable};
use slog::Logger;

use crate::error::{Result, Error};

use crate::db::{access, Databases, MachineDB};
use crate::db::access::AccessControl;
use crate::db::machine::{MachineIdentifier, MachineState, Status};
use crate::db::user::{User, UserData, UserId};

use crate::network::MachineMap;
use crate::space;

pub struct Machines {
    machines: Vec<Machine>
}

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

// Access data of one machine efficiently, using getters/setters for data stored in LMDB backed
// memory
#[derive(Debug, Clone)]
pub struct Machine {
    pub id: MachineIdentifier,
    pub desc: MachineDescription,

    access_control: Arc<AccessControl>,

    inner: Arc<Mutex<Inner>>,
}

impl Machine {
    pub fn new(
        inner: Inner,
        id: MachineIdentifier,
        desc: MachineDescription,
        access_control: Arc<AccessControl>
        ) -> Self
    {
        Self { 
            id,
            inner: Arc::new(Mutex::new(inner)),
            desc,
            access_control,
        }
    }

    pub fn construct(
        id: MachineIdentifier,
        desc: MachineDescription,
        state: MachineState,
        db: Arc<MachineDB>,
        access_control: Arc<AccessControl>,
        ) -> Machine
    {
        Self::new(Inner::new(id.clone(), state, db), id, desc, access_control)
    }

    pub fn do_state_change(&self, new_state: MachineState) 
        -> BoxFuture<'static, Result<()>>
    {
        let this = self.clone();

        let f = async move {
            let mut guard = this.inner.lock().await;
            guard.do_state_change(new_state);
            return Ok(())
        };

        Box::pin(f)
    }

    pub fn request_state_change(&mut self, user: Option<&User>, new_state: MachineState)
        -> BoxFuture<'static, Result<ReturnToken>>
    {
        let inner = self.inner.clone();
        Box::pin(async move {
            Ok(ReturnToken::new(inner))
        })
    }

    pub async fn get_status(&self) -> Status {
        let guard = self.inner.lock().await;
        guard.state.get_cloned().state
    }

    pub fn signal(&self) -> impl Signal<Item=MachineState> {
        let guard = self.inner.try_lock().unwrap();
        guard.signal()
    }

    pub fn get_inner(&self) -> Arc<Mutex<Inner>> {
        self.inner.clone()
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

    /// The state of the machine as bffh thinks the machine *should* be in.
    ///
    /// This is a Signal generator. Subscribers to this signal will be notified of changes. In the
    /// case of an actor it should then make sure that the real world matches up with the set state
    state: Mutable<MachineState>,
    reset: Option<MachineState>,

    previous: Option<UserId>,
    db: Arc<MachineDB>,
}

impl Inner {
    pub fn new(id: MachineIdentifier, state: MachineState, db: Arc<MachineDB>) -> Inner {
        Inner {
            id,
            state: Mutable::new(state),
            reset: None,
            previous: None,
            db,
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

    fn replace_state(&mut self, new_state: MachineState) -> MachineState {
        self.db.put(&self.id, &new_state);
        self.state.replace(new_state)
    }

    pub fn do_state_change(&mut self, new_state: MachineState) {
        let old_state = self.replace_state(new_state);

        // Set "previous user" if state change warrants it
        match old_state.state {
            Status::InUse(ref user) => {
                self.previous = user.clone();
            },
            Status::ToCheck(ref user) => {
                self.previous = Some(user.clone());
            },
            _ => {},
        }

        self.reset.replace(old_state);
    }

    pub fn read_state(&self) -> ReadOnlyMutable<MachineState> {
        self.state.read_only()
    }

    pub fn get_signal(&self) -> impl Signal {
        self.state.signal_cloned()
    }

    pub fn reset_state(&mut self) {
        let previous_state = self.read_state();
        let state_lock = previous_state.lock_ref();
        // Only update previous user if state changed from InUse or ToCheck to whatever.
        match state_lock.state {
            Status::InUse(ref user) => {
                self.previous = user.clone();
            },
            Status::ToCheck(ref user) => {
                self.previous = Some(user.clone());
            },
            _ => {},
        }
        drop(state_lock);

        if let Some(state) = self.reset.take() {
            self.replace_state(state);
        } else {
            // Default to Free
            self.replace_state(MachineState::free());
        }
    }

    pub fn get_previous(&self) -> &Option<UserId> {
        &self.previous
    }
}

//pub type ReturnToken = futures::channel::oneshot::Sender<()>;
pub struct ReturnToken {
    f: Option<BoxFuture<'static, ()>>,
}

impl ReturnToken {
    pub fn new(inner: Arc<Mutex<Inner>>) -> Self {
        let f = async move {
            let mut guard = inner.lock().await;
            guard.reset_state();
        };

        Self { f: Some(Box::pin(f)) }
    }
}

impl Future for ReturnToken {
    type Output = (); // FIXME: This should probably be a Result<(), Error>

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let mut this = &mut *self;

        match this.f.as_mut().map(|f| Future::poll(Pin::new(f), cx)) {
            None => Poll::Ready(()), // TODO: Is it saner to return Pending here? This can only happen after the future completed
            Some(Poll::Pending) => Poll::Pending,
            Some(Poll::Ready(())) => {
                let _ = this.f.take(); // Remove the future to not poll after completion
                Poll::Ready(())
            }
        }
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

    #[serde(default)]
    #[serde(flatten)]
    pub wiki: Option<String>,

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

pub fn load(config: &crate::config::Config, db: Databases, log: &Logger)
    -> Result<MachineMap> 
{
    let mut map = config.machines.clone();
    let access_control = db.access;
    let db = db.machine;

    let it = map.drain()
        .map(|(k,v)| {
            // TODO: Read state from the state db
            if let Some(state) = db.get(&k).unwrap() {
                debug!(log, "Loading old state from db for {}: {:?}", &k, &state);
                (k.clone(),
                 Machine::construct(
                    k,
                    v,
                    state,
                    db.clone(),
                    access_control.clone()
                 ))
            } else {
                debug!(log, "No old state found in db for {}, creating new.", &k);
                (k.clone(),
                 Machine::construct(
                     k,
                     v,
                     MachineState::new(),
                     db.clone(),
                     access_control.clone(),
                 ))
            }
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
