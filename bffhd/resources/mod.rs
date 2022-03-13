use crate::resources::modules::fabaccess::{MachineState, Status};
use crate::resources::state::State;
use crate::session::SessionHandle;
use crate::users::User;

pub mod claim;
pub mod db;
pub mod driver;
pub mod search;
pub mod state;

pub mod modules;

pub struct PermissionDenied;

#[derive(Clone)]
pub struct Resource {}

impl Resource {
    pub fn get_state(&self) -> MachineState {
        unimplemented!()
    }

    fn set_state(&self, state: MachineState) {
        unimplemented!()
    }

    fn set_previous_user(&self, user: User) {
        unimplemented!()
    }

    pub async fn try_update(&self, session: SessionHandle, new: MachineState) {
        let old = self.get_state();
        let user = session.get_user();

        if session.has_manage(self) // Default allow for managers

            || (session.has_write(self) // Decision tree for writers
                && match (old.state, &new.state) {
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
                if who == user => true,

                // Un-reserving things we reserved is okay
                (Status::Reserved(whom), Status::Free)
                if user == whom => true,
                // Using things that we've reserved is okay. But the person requesting
                // that has to be the person that reserved the machine. Otherwise
                // somebody could make a machine reserved by a different user as used by
                // that different user but use it themself.
                (Status::Reserved(whom), Status::InUse(who))
                if user == whom && who == &whom => true,

                // Default is deny.
                _ => false
            })

            // Default permissions everybody has
            || match (old.state, &new.state) {
                // Returning things we've been using is okay. This includes both if
                // they're being freed or marked as to be checked.
                (Status::InUse(who), Status::Free | Status::ToCheck(_)) if who == user => true,

                // Un-reserving things we reserved is okay
                (Status::Reserved(whom), Status::Free) if user == whom => true,

                // Default is deny.
                _ => false,
            }
        {
            self.set_state(new);
        }
    }

    pub async fn give_back(&self, session: SessionHandle) {
        if let Status::InUse(user) = self.get_state().state {
            if user == session.get_user() {
                self.set_state(MachineState::free());
                self.set_previous_user(user);
            }
        }
    }

    pub async fn force_set(&self, new: MachineState) {
        unimplemented!()
    }

    pub fn visible(&self, session: &SessionHandle) -> bool {
        session.has_disclose(self) || self.is_owned_by(session.get_user())
    }

    pub fn is_owned_by(&self, owner: User) -> bool {
        match self.get_state().state {
            Status::Free | Status::Disabled => false,

            Status::InUse(user)
            | Status::ToCheck(user)
            | Status::Blocked(user)
            | Status::Reserved(user) => user == owner,
        }
    }
}
