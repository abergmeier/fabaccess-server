use std::ops::Deref;
use std::iter::FromIterator;
use std::sync::Arc;
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

use crate::error::{Result, Error};

use crate::db::access::{AccessControl, PrivilegesBuf, PermissionBuf};
use crate::db::machine::{MachineIdentifier, MachineState, Status};
use crate::db::user::{User, UserData, UserId};

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
    pub id: uuid::Uuid,
    pub desc: MachineDescription,

    inner: Arc<Mutex<Inner>>,
}

impl Machine {
    pub fn new(inner: Inner, desc: MachineDescription, ) -> Self {
        Self { 
            id: uuid::Uuid::default(),
            inner: Arc::new(Mutex::new(inner)),
            desc,
        }
    }

    pub fn construct
        ( id: MachineIdentifier
        , desc: MachineDescription
        , state: MachineState
        ) -> Machine
    {
        Self::new(Inner::new(id, state), desc)
    }

    fn match_perm(&self, status: &Status) -> Option<&PermissionBuf> {
        let p = self.desc.privs;
        match status {
            // If you were allowed to use it you're allowed to give it back
            Status::Free 
            | Status::ToCheck(_)
                => None,

            Status::Blocked(_) 
            | Status::Disabled 
            | Status::Reserved(_) 
                => Some(&p.manage),

            Status::InUse(_) => Some(&p.write),
        }
    }

    pub fn request_state_change(&self, new_state: MachineState, access: AccessControl, user: &User)
        -> BoxFuture<'static, Result<()>>
    {
        let this = self.clone();
        let perm = self.match_perm(&new_state.state);
        let grant = perm.map(|p| access.check(&user.data, p).unwrap_or(false));

        let uid = user.id.clone();
        // is it a return
        let is_ret = new_state.state == Status::Free;
        // is it a (normal) write /the user is allowed to do/?
        let is_wri = new_state.state == Status::InUse(Some(uid))
                     && access.check(&user.data, self.desc.privs.write).unwrap_or(false);

        let f = async move {
            let mut guard = this.inner.lock().await;
            // either e.g. InUse(<myself>) => Free or I'm allowed to overwrite
            if (is_ret && guard.is_self(uid))
            || (is_wri && guard.is_free())
            || grant.unwrap_or(false)
            {
                guard.do_state_change(new_state);
            }
            return Ok(())
        };

        Box::pin(f)
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
///
/// Machines have a rather complex state machine since they have to be eventually consistent and
/// can fail at any point in time (e.g. because power cuts out suddenly, a different task on this
/// thread panics, some loaded code produces a segfault, ...)
pub struct Inner {
    /// Globally unique machine readable identifier
    pub id: MachineIdentifier,

    /// The state of the machine as bffh thinks the machine *should* be in.
    ///
    /// This is a Signal generator. Subscribers to this signal will be notified of changes. In the
    /// case of an actor it should then make sure that the real world matches up with the set state
    state: Mutable<MachineState>,
    reset: Option<MachineState>,
}

impl Inner {
    pub fn new ( id: MachineIdentifier
               , state: MachineState
               ) -> Inner 
    {
        Inner {
            id,
            state: Mutable::new(state),
            reset: None,
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

    pub fn do_state_change(&mut self, new_state: MachineState) {
            let old_state = self.state.replace(new_state);
            self.reset.replace(old_state);
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

    pub fn is_self(&mut self, uid: UserId) -> bool {
        match self.read_state().get_cloned().state {
            Status::InUse(u) if u == uid => true,
            _ => false,
        }
    }

    pub fn is_free(&mut self) -> bool {
        match self.read_state().get_cloned().state {
            Status::Free => true,
            _ => false,
        }
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

    /// The permission required
    #[serde(flatten)]
    pub privs: PrivilegesBuf,
}

impl MachineDescription {
    pub fn load_file<P: AsRef<Path>>(path: P) -> Result<HashMap<MachineIdentifier, MachineDescription>> {
        let content = fs::read(path)?;
        Ok(toml::from_slice(&content[..])?)
    }
}

pub fn load(config: &crate::config::Config)
    -> Result<MachineMap> 
{
    let mut map = config.machines.clone();

    let it = map.drain()
        .map(|(k,v)| {
            // TODO: Read state from the state db
            (v.name.clone(), Machine::construct(k, v, MachineState::new()))
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
