use std::sync::Arc;
use std::time::Duration;

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
        mut results: info::GetMachineInfoExtendedResults,
    ) -> Promise<(), capnp::Error> {
        let machine = self.machine.get_inner();
        let perms = self.perms.clone();
        let f = async move {
            if perms.manage {
                let mut builder = results.get();
                let mut extinfo = builder.init_machine_info_extended();
                let guard = machine.lock().await;

                // "previous" user
                if let Some(user) = guard.get_previous() {
                    let mut previous = extinfo.reborrow().init_transfer_user();
                    previous.set_username(&user.uid);
                }

                let state = guard.read_state();
                let state_lock = state.lock_ref();
                match state_lock.state {
                    Status::Free => {}
                    Status::InUse(ref user) => if user.is_some() {
                        let user = user.as_ref().unwrap();
                        let mut current = extinfo.init_current_user();
                        current.set_username(&user.uid);
                    }
                    Status::ToCheck(ref user) => {
                        let mut current = extinfo.init_current_user();
                        current.set_username(&user.uid);
                    }
                    Status::Blocked(ref user) => {
                        let mut current = extinfo.init_current_user();
                        current.set_username(&user.uid);
                    }
                    Status::Disabled => {}
                    Status::Reserved(ref user) => {
                        let mut current = extinfo.init_current_user();
                        current.set_username(&user.uid);
                    }
                }
            }

            Ok(())
        };

        let g = smol::future::race(f, smol::Timer::after(Duration::from_secs(4))
            .map(|_| Err(capnp::Error::failed("Waiting for machine lock timed out!".to_string()))));

        Promise::from_future(g)
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
            let mut ok = false;
            {
                match { guard.read_state().lock_ref().state.clone() } {
                    Status::Free => {
                        ok = true;
                    },
                    Status::Reserved(ref whom) => {
                        // If it's reserved for us or we're allowed to take over
                        if &userid == whom {
                            ok = true;
                        }
                    },
                    _ => { }
                }
            }

            if ok {
                guard.do_state_change(MachineState::used(Some(userid)));
            }

            Ok(())
        };

        let g = smol::future::race(f, smol::Timer::after(Duration::from_secs(4))
            .map(|_| Err(capnp::Error::failed("Waiting for machine lock timed out!".to_string()))));

        Promise::from_future(g)
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
            let mut ok = false;
            {
                match { guard.read_state().lock_ref().state.clone() } {
                    Status::InUse(ref whom) => {
                        if &Some(userid) == whom {
                            ok = true;
                        }
                    },
                    _ => { }
                }
            }

            if ok {
                guard.reset_state()
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
