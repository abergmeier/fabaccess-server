use std::sync::Arc;

use capnp::capability::Promise;
use capnp::Error;

use futures::FutureExt;

use crate::db::access::{PrivilegesBuf, PermRule};

use crate::db::machine::Status;
use crate::machine::Machine as NwMachine;
use crate::schema::machine_capnp::machine::*;

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

pub struct Machine {
    perms: Perms,
    machine: NwMachine,
}

impl Machine {
    pub fn new(perms: Perms, machine: NwMachine) -> Self {
        Self { perms, machine }
    }
}

impl info::Server for Machine {
    fn get_machine_info_extended(
        &mut self,
        _: info::GetMachineInfoExtendedParams,
        _results: info::GetMachineInfoExtendedResults,
    ) -> capnp::capability::Promise<(), capnp::Error> {
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
    ) -> capnp::capability::Promise<(), capnp::Error> {
        Promise::err(capnp::Error::unimplemented("Reservations are unavailable".to_string()))
    }

    fn get_property_list(
        &mut self,
        _: info::GetPropertyListParams,
        mut results: info::GetPropertyListResults,
    ) -> capnp::capability::Promise<(), capnp::Error> {
        Promise::err(capnp::Error::unimplemented("Extended Properties are unavailable".to_string()))
    }
}

impl use_::Server for Machine {
    fn use_(
        &mut self,
        _: use_::UseParams,
        _: use_::UseResults
    ) -> capnp::capability::Promise<(), capnp::Error> {
        Promise::ok(())
    }
}

impl in_use::Server for Machine {
}

impl transfer::Server for Machine {
}

impl check::Server for Machine {
}

impl manage::Server for Machine {
}

impl admin::Server for Machine {
}
