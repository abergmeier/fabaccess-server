use std::sync::Arc;

use capnp::capability::Promise;
use capnp::Error;

use futures::FutureExt;

use crate::db::access::{PrivilegesBuf, PermRule};
use crate::db::user::UserId;
use crate::db::machine::{Status, MachineState};
use crate::machine::Machine as NwMachine;
use crate::schema::machine_capnp::machine::*;
use crate::schema::machine_capnp::machine::MachineState as APIMState;

#[derive(Clone, Copy)]
pub struct Perms {
    pub disclose: bool,
    pub read: bool,
    pub write: bool,
    pub manage: bool,
}

impl Perms {
    pub fn get_for<'a, I: Iterator<Item=&'a PermRule>>(privs: &'a PrivilegesBuf, rules: I) -> Self {
        let mut disclose = false;
        let mut read = false;
        let mut write = false;
        let mut manage = false;
        for rule in rules {
            if rule.match_perm(&privs.disclose) {
                disclose = true;
            }
            if rule.match_perm(&privs.read) {
                read = true;
            }
            if rule.match_perm(&privs.write) {
                write = true;
            }
            if rule.match_perm(&privs.manage) {
                manage = true;
            }
        }

        Self { disclose, read, write, manage }
    }
}

#[derive(Clone)]
pub struct Machine {
    userid: UserId,
    perms: Perms,
    machine: NwMachine,
}

impl Machine {
    pub fn new(userid: UserId, perms: Perms, machine: NwMachine) -> Self {
        Self { userid, perms, machine }
    }
}

impl info::Server for Machine {
    fn get_machine_info_extended(
        &mut self,
        _: info::GetMachineInfoExtendedParams,
        _results: info::GetMachineInfoExtendedResults,
    ) -> Promise<(), capnp::Error> {
        /*if self.perms.manage {
            let mut builder = results.get();
            let mut extinfo = builder.init_machine_info_extended();
            let mut current = extinfo.init_current_user();
            // FIXME fill user
        }
        Promise::ok(())*/

        Promise::err(capnp::Error::unimplemented("Extended Infos are unavailable".to_string()))
    }

    fn get_reservation_list(
        &mut self,
        _: info::GetReservationListParams,
        mut results: info::GetReservationListResults,
    ) -> Promise<(), capnp::Error> {
        Promise::err(capnp::Error::unimplemented("Reservations are unavailable".to_string()))
    }

    fn get_property_list(
        &mut self,
        _: info::GetPropertyListParams,
        mut results: info::GetPropertyListResults,
    ) -> Promise<(), capnp::Error> {
        Promise::err(capnp::Error::unimplemented("Extended Properties are unavailable".to_string()))
    }
}

impl use_::Server for Machine {
    fn use_(
        &mut self,
        _: use_::UseParams,
        _: use_::UseResults
    ) -> Promise<(), capnp::Error> {
        let machine = self.machine.get_inner();
        let userid = self.userid.clone();
        let f = async move {
            let mut guard = machine.lock().await;
            match guard.read_state().lock_ref().state {
                Status::Free => {
                    guard.do_state_change(MachineState::used(Some(userid)));
                },
                Status::Reserved(ref whom) => {
                    // If it's reserved for us or we're allowed to take over
                    if &userid == whom {
                        guard.do_state_change(MachineState::used(Some(userid)));
                    }
                },
                _ => { }
            }

            Ok(())
        };

        Promise::from_future(f)
    }
}

impl in_use::Server for Machine {
    fn give_back(
        &mut self,
        _:in_use::GiveBackParams,
        _:in_use::GiveBackResults
    ) -> Promise<(), capnp::Error> {
        let machine = self.machine.get_inner();
        let userid = self.userid.clone();
        let f = async move {
            let mut guard = machine.lock().await;
            match guard.read_state().lock_ref().state {
                Status::InUse(ref whom) => {
                    if &Some(userid) == whom {
                        guard.reset_state()
                    }
                },
                _ => {}
            }

            Ok(())
        };

        Promise::from_future(f)
    }
}

impl transfer::Server for Machine {
}

impl check::Server for Machine {
}

impl manage::Server for Machine {
    fn force_free(&mut self,
        _: manage::ForceFreeParams,
        _: manage::ForceFreeResults
    ) -> Promise<(), capnp::Error> {
        let machine = self.machine.get_inner();
        let f = async move {
            let mut guard = machine.lock().await;
            guard.do_state_change(MachineState::free());
            Ok(())
        };
        Promise::from_future(f)
    }

    fn force_use(&mut self,
        _: manage::ForceUseParams,
        _: manage::ForceUseResults
        ) -> Promise<(), capnp::Error> {
        let machine = self.machine.get_inner();
        let f = async move {
            let mut guard = machine.lock().await;
            guard.do_state_change(MachineState::used(None));
            Ok(())
        };
        Promise::from_future(f)
    }

    fn block(&mut self,
        _:manage::BlockParams,
        _:manage::BlockResults
        ) -> Promise<(), capnp::Error> {
        let machine = self.machine.get_inner();
        let uid = self.userid.clone();
        let f = async move {
            let mut guard = machine.lock().await;
            guard.do_state_change(MachineState::blocked(uid));
            Ok(())
        };
        Promise::from_future(f)
    }

    fn disabled(&mut self,
        _:manage::DisabledParams,
        _:manage::DisabledResults
        ) -> Promise<(), capnp::Error> {
        let machine = self.machine.get_inner();
        let f = async move {
            let mut guard = machine.lock().await;
            guard.do_state_change(MachineState::disabled());
            Ok(())
        };
        Promise::from_future(f)
    }
}

impl admin::Server for Machine {
    fn force_set_state(&mut self,
        params: admin::ForceSetStateParams,
        _:admin::ForceSetStateResults
        ) -> Promise<(), capnp::Error> {
        let uid = self.userid.clone();
        let state = match pry!(pry!(params.get()).get_state()) {
            APIMState::Free => MachineState::free(),
            APIMState::Blocked => MachineState::blocked(uid),
            APIMState::Disabled => MachineState::disabled(),
            APIMState::InUse => MachineState::used(Some(uid)),
            APIMState::Reserved => MachineState::reserved(uid),
            APIMState::ToCheck => MachineState::check(uid),
        };
        let machine = self.machine.get_inner();
        let f = async move {
            let mut guard = machine.lock().await;
            guard.do_state_change(state);
            Ok(())
        };
        Promise::from_future(f)
    }

}
