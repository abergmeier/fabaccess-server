
use std::sync::Arc;
use futures_signals::signal::{Mutable, Signal, SignalExt};
use lmdb::RoTransaction;
use rkyv::Archived;
use crate::authorization::permissions::PrivilegesBuf;
use crate::config::MachineDescription;
use crate::db::LMDBorrow;
use crate::resources::modules::fabaccess::{MachineState, Status};
use crate::resources::state::db::StateDB;
use crate::resources::state::State;
use crate::session::SessionHandle;
use crate::users::UserRef;

pub mod claim;
pub mod db;
pub mod driver;
pub mod search;
pub mod state;

pub mod modules;

pub struct PermissionDenied;

pub(crate) struct Inner {
    id: String,
    db: Arc<StateDB>,
    signal: Mutable<State>,
    desc: MachineDescription,
}
impl Inner {
    pub fn new(id: String, db: Arc<StateDB>, desc: MachineDescription) -> Self {
        let state = if let Some(previous) = db.get_output(id.as_bytes()).unwrap() {
            let state = MachineState::from(&previous);
            tracing::info!(%id, ?state, "Found previous state");
            state
        } else {
            tracing::info!(%id, "No previous state, defaulting to `free`");
            let state = MachineState::used(UserRef::new("test".to_string()), Some(UserRef::new
                ("prev".to_string())));
            let update = state.to_state();
            db.update(id.as_bytes(), &update, &update).unwrap();
            state
        };
        let signal = Mutable::new(state.to_state());

        Self { id, db, signal, desc }
    }

    pub fn signal(&self) -> impl Signal<Item=State> {
        Box::pin(self.signal.signal_cloned().dedupe_cloned())
    }

    fn get_state(&self) -> MachineState {
        MachineState::from(&self.db.get_output(self.id.as_bytes()).unwrap().unwrap())
    }

    fn get_raw_state(&self) -> Option<LMDBorrow<RoTransaction, Archived<State>>> {
        self.db.get_output(self.id.as_bytes()).unwrap()
    }

    fn set_state(&self, state: MachineState) {
        let span = tracing::debug_span!("set", id = %self.id, ?state, "Updating state");
        let _guard = span.enter();
        tracing::debug!("Updating state");
        tracing::trace!("Updating DB");
        let update = state.to_state();
        self.db.update(self.id.as_bytes(), &update, &update).unwrap();
        tracing::trace!("Updated DB, sending update signal");
        self.signal.set(update);
        tracing::trace!("Sent update signal");
    }
}

#[derive(Clone)]
pub struct Resource {
    inner: Arc<Inner>
}

impl Resource {
    pub(crate) fn new(inner: Arc<Inner>) -> Self {
        Self { inner }
    }

    pub fn get_raw_state(&self) -> Option<LMDBorrow<RoTransaction, Archived<State>>> {
        self.inner.get_raw_state()
    }

    pub fn get_state(&self) -> MachineState {
        self.inner.get_state()
    }

    pub fn get_id(&self) -> &str {
        &self.inner.id
    }

    pub fn get_signal(&self) -> impl Signal<Item=State> {
        self.inner.signal()
    }

    pub fn get_required_privs(&self) -> &PrivilegesBuf {
        &self.inner.desc.privs
    }

    fn set_state(&self, state: MachineState) {
        self.inner.set_state(state)
    }

    fn set_status(&self, state: Status) {
        let old = self.inner.get_state();
        let new = MachineState { state, .. old };
        self.set_state(new);
    }

    pub async fn try_update(&self, session: SessionHandle, new: Status) {
        let old = self.get_state();
        let user = session.get_user();

        if session.has_manage(self) // Default allow for managers

            || (session.has_write(self) // Decision tree for writers
                && match (&old.state, &new) {
                // Going from available to used by the person requesting is okay.
                (Status::Free, Status::InUse(who))
                // Check that the person requesting does not request for somebody else.
                // *That* is manage privilege.
                if who == &user => true,

                // Reserving things for ourself is okay.
                (Status::Free, Status::Reserved(whom))
                if &user == whom => true,

                // Returning things we've been using is okay. This includes both if
                // they're being freed or marked as to be checked.
                (Status::InUse(who), Status::Free | Status::ToCheck(_))
                if who == &user => true,

                // Un-reserving things we reserved is okay
                (Status::Reserved(whom), Status::Free)
                if whom == &user => true,
                // Using things that we've reserved is okay. But the person requesting
                // that has to be the person that reserved the machine. Otherwise
                // somebody could make a machine reserved by a different user as used by
                // that different user but use it themself.
                (Status::Reserved(whom), Status::InUse(who))
                if whom == &user && who == whom => true,

                // Default is deny.
                _ => false
            })

            // Default permissions everybody has
            || match (&old.state, &new) {
                // Returning things we've been using is okay. This includes both if
                // they're being freed or marked as to be checked.
                (Status::InUse(who), Status::Free | Status::ToCheck(_)) if who == &user => true,

                // Un-reserving things we reserved is okay
                (Status::Reserved(whom), Status::Free) if whom == &user => true,

                // Default is deny.
                _ => false,
            }
        {
            self.set_status(new);
        }
    }

    pub async fn give_back(&self, session: SessionHandle) {
        if let Status::InUse(user) = self.get_state().state {
            if user == session.get_user() {
                self.set_state(MachineState::free(Some(user)));
            }
        }
    }

    pub async fn force_set(&self, new: Status) {
        self.set_status(new);
    }

    pub fn visible(&self, session: &SessionHandle) -> bool {
        session.has_disclose(self) || self.is_owned_by(session.get_user())
    }

    pub fn is_owned_by(&self, owner: UserRef) -> bool {
        match self.get_state().state {
            Status::Free | Status::Disabled => false,

            Status::InUse(user)
            | Status::ToCheck(user)
            | Status::Blocked(user)
            | Status::Reserved(user) => user == owner,
        }
    }
}
