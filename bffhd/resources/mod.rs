use rkyv::Infallible;
use std::ops::Deref;
use std::sync::Arc;
use futures_signals::signal::{Mutable, Signal};

use rkyv::{Archived, Deserialize};
use rkyv::option::ArchivedOption;
use rkyv::ser::Serializer;
use rkyv::ser::serializers::AllocSerializer;
use crate::audit::AUDIT;
use crate::authorization::permissions::PrivilegesBuf;
use crate::config::MachineDescription;
use crate::db::ArchivedValue;
use crate::resources::modules::fabaccess::{MachineState, Status, ArchivedStatus};
use crate::resources::state::db::StateDB;
use crate::resources::state::State;
use crate::session::SessionHandle;
use crate::users::UserRef;

pub mod db;
pub mod search;
pub mod state;

pub mod modules;

pub struct PermissionDenied;

pub(crate) struct Inner {
    id: String,
    db: StateDB,
    signal: Mutable<ArchivedValue<State>>,
    desc: MachineDescription,
}
impl Inner {
    pub fn new(id: String, db: StateDB, desc: MachineDescription) -> Self {
        let state = if let Some(previous) = db.get(id.as_bytes()).unwrap() {
            tracing::info!(%id, ?previous, "Found previous state");
            previous
        } else {
            let state = MachineState::free(None);
            tracing::info!(%id, ?state, "No previous state found, setting default");

            let update = state.to_state();

            let mut serializer = AllocSerializer::<1024>::default();
            serializer.serialize_value(&update).expect("failed to serialize new default state");
            let val = ArchivedValue::new(serializer.into_serializer().into_inner());
            db.put(&id.as_bytes(), &val).unwrap();
            val
        };
        let signal = Mutable::new(state);

        Self { id, db, signal, desc }
    }

    pub fn signal(&self) -> impl Signal<Item=ArchivedValue<State>> {
        Box::pin(self.signal.signal_cloned())
    }

    fn get_state(&self) -> ArchivedValue<State> {
        self.db.get(self.id.as_bytes())
            .expect("lmdb error")
            .expect("state should never be None")
    }

    fn get_state_ref(&self) -> impl Deref<Target=ArchivedValue<State>> + '_ {
        self.signal.lock_ref()
    }

    fn set_state(&self, state: ArchivedValue<State>) {
        let span = tracing::debug_span!("set_state", id = %self.id, ?state);
        let _guard = span.enter();
        tracing::debug!("Updating state");

        tracing::trace!("Updating DB");
        self.db.put(&self.id.as_bytes(), &state).unwrap();
        tracing::trace!("Updated DB, sending update signal");

        AUDIT.get().unwrap().log(self.id.as_str(), &format!("{}", state));

        self.signal.set(state);
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

    pub fn get_state(&self) -> ArchivedValue<State> {
        self.inner.get_state()
    }

    pub fn get_state_ref(&self) -> impl Deref<Target=ArchivedValue<State>> + '_ {
        self.inner.get_state_ref()
    }

    pub fn get_id(&self) -> &str {
        &self.inner.id
    }

    pub fn get_name(&self) -> &str {
        self.inner.desc.name.as_str()
    }

    pub fn get_signal(&self) -> impl Signal<Item=ArchivedValue<State>> {
        self.inner.signal()
    }

    pub fn get_required_privs(&self) -> &PrivilegesBuf {
        &self.inner.desc.privs
    }

    pub fn get_description(&self) -> &MachineDescription {
        &self.inner.desc
    }

    pub fn get_current_user(&self) -> Option<UserRef> {
        let state = self.get_state_ref();
        let state: &Archived<State> = state.as_ref();
        match &state.inner.state {
            ArchivedStatus::Blocked(user) |
            ArchivedStatus::InUse(user) |
            ArchivedStatus::Reserved(user) |
            ArchivedStatus::ToCheck(user) => {
                let user = Deserialize::<UserRef, _>::deserialize(user, &mut Infallible).unwrap();
                Some(user)
            },
            _ => None,
        }
    }

    pub fn get_previous_user(&self) -> Option<UserRef> {
        let state = self.get_state_ref();
        let state: &Archived<State> = state.as_ref();
        if let ArchivedOption::Some(user) = &state.inner.previous {
            let user = Deserialize::<UserRef, _>::deserialize(user, &mut Infallible).unwrap();
            Some(user)
        } else {
            None
        }
    }

    fn set_state(&self, state: MachineState) {
        let mut serializer = AllocSerializer::<1024>::default();
        serializer.serialize_value(&state);
        let archived = ArchivedValue::new(serializer.into_serializer().into_inner());
        self.inner.set_state(archived)
    }

    fn set_status(&self, state: Status) {
        let old = self.inner.get_state();
        let oldref: &Archived<State> = old.as_ref();
        let previous: &Archived<Option<UserRef>> = &oldref.inner.previous;
        let previous = Deserialize::<Option<UserRef>, _>::deserialize(previous, &mut rkyv::Infallible)
            .expect("Infallible deserializer failed");
        let new = MachineState { state, previous };
        self.set_state(new);
    }

    pub async fn try_update(&self, session: SessionHandle, new: Status) {
        let old = self.get_state();
        let old: &Archived<State> = old.as_ref();
        let user = session.get_user_ref();

        if session.has_manage(self) // Default allow for managers

            || (session.has_write(self) // Decision tree for writers
                && match (&old.inner.state, &new) {
                // Going from available to used by the person requesting is okay.
                (ArchivedStatus::Free, Status::InUse(who))
                // Check that the person requesting does not request for somebody else.
                // *That* is manage privilege.
                if who == &user => true,

                // Reserving things for ourself is okay.
                (ArchivedStatus::Free, Status::Reserved(whom))
                if &user == whom => true,

                // Returning things we've been using is okay. This includes both if
                // they're being freed or marked as to be checked.
                (ArchivedStatus::InUse(who), Status::Free | Status::ToCheck(_))
                if who == &user => true,

                // Un-reserving things we reserved is okay
                (ArchivedStatus::Reserved(whom), Status::Free)
                if whom == &user => true,
                // Using things that we've reserved is okay. But the person requesting
                // that has to be the person that reserved the machine. Otherwise
                // somebody could make a machine reserved by a different user as used by
                // that different user but use it themself.
                (ArchivedStatus::Reserved(whom), Status::InUse(who))
                if whom == &user && who == whom => true,

                // Default is deny.
                _ => false
            })

            // Default permissions everybody has
            || match (&old.inner.state, &new) {
                // Returning things we've been using is okay. This includes both if
                // they're being freed or marked as to be checked.
                (ArchivedStatus::InUse(who), Status::Free | Status::ToCheck(_)) if who == &user => true,

                // Un-reserving things we reserved is okay
                (ArchivedStatus::Reserved(whom), Status::Free) if whom == &user => true,

                // Default is deny.
                _ => false,
            }
        {
            self.set_status(new);
        }
    }

    pub async fn give_back(&self, session: SessionHandle) {
        let state = self.get_state();
        let s: &Archived<State> = state.as_ref();
        let i: &Archived<MachineState> = &s.inner;
        if let ArchivedStatus::InUse(user) = &i.state {
            let current = session.get_user_ref();
            if user == &current {
                self.set_state(MachineState::free(Some(current)));
            }
        }
    }

    pub async fn force_set(&self, new: Status) {
        self.set_status(new);
    }

    pub fn visible(&self, session: &SessionHandle) -> bool {
        session.has_disclose(self) || self.is_owned_by(session.get_user_ref())
    }

    pub fn is_owned_by(&self, owner: UserRef) -> bool {
        match &self.get_state().as_ref().inner.state {
            ArchivedStatus::Free | ArchivedStatus::Disabled => false,

            ArchivedStatus::InUse(user)
            | ArchivedStatus::ToCheck(user)
            | ArchivedStatus::Blocked(user)
            | ArchivedStatus::Reserved(user) => user == &owner,
        }
    }
}
