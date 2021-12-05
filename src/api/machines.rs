use std::sync::Arc;
use std::cell::RefCell;
use std::rc::Rc;
use std::ops::Deref;

use capnp::capability::Promise;
use capnp::Error;

use crate::db::machine::Status;
use crate::api::machine::*;
use crate::schema::machine_capnp::machine::MachineState;
use crate::schema::machinesystem_capnp::machine_system;
use crate::schema::machinesystem_capnp::machine_system::info as machines;
use crate::network::Network;
use crate::db::user::UserId;
use crate::db::access::{PermRule, admin_perm};
use crate::connection::Session;

/// An implementation of the `Machines` API
#[derive(Clone)]
pub struct Machines {
    session: Rc<RefCell<Option<Session>>>,
    network: Arc<Network>,
}

impl Machines {
    pub fn new(session: Rc<RefCell<Option<Session>>>, network: Arc<Network>) -> Self {
        Self { session, network }
    }
}

impl machine_system::Server for Machines {
    // This function shouldn't exist. See fabaccess-api issue #16
    fn info(&mut self,
        _:machine_system::InfoParams,
        mut results: machine_system::InfoResults
        ) -> capnp::capability::Promise<(), capnp::Error>
    {
        results.get().set_info(capnp_rpc::new_client(self.clone()));
        Promise::ok(())
    }
}

impl machines::Server for Machines {
    fn get_machine_list(&mut self,
        _params: machines::GetMachineListParams,
        mut results: machines::GetMachineListResults)
        -> Promise<(), Error>
    {
        let rc = Rc::clone(&self.session);
        let session = self.session.borrow();
        if session.deref().is_some() {
            let v: Vec<(String, crate::machine::Machine)> = self.network.machines.iter()
                .filter(|(_name, machine)| {
                    let required_disclose = &machine.desc.privs.disclose;
                    for perm_rule in session.as_ref().unwrap().perms.iter() {
                        if perm_rule.match_perm(required_disclose) {
                            return true;
                        }
                    }

                    false
                })
                .map(|(n,m)| (n.clone(), m.clone()))
                .collect();

            let f = async move {
                let session = rc.borrow();
                let user = &session.as_ref().unwrap().authzid;
                let permissions = &session.as_ref().unwrap().perms;

                let mut machines = results.get().init_machine_list(v.len() as u32);
                for (i, (name, machine)) in v.into_iter().enumerate() {
                    let perms = Perms::get_for(&machine.desc.privs, permissions.iter());

                    let mut builder = machines.reborrow().get(i as u32);
                    builder.set_id(&name);
                    builder.set_name(&machine.desc.name);
                    if let Some(ref desc) = machine.desc.description {
                        builder.set_description(desc);
                    }
                    if let Some(ref wiki) = machine.desc.wiki {
                        builder.set_wiki(wiki);
                    }
                    builder.set_urn(&format!("urn:fabaccess:resource:{}", &name));

                    let machineapi = Machine::new(user.clone(), perms, machine.clone());

                    let state = machine.get_status().await;
                    let s = match state {
                        Status::Free => MachineState::Free,
                        Status::Disabled => MachineState::Disabled,
                        Status::Blocked(_) => MachineState::Blocked,
                        Status::InUse(ref u) => {
                            if let Some(owner) = u.as_ref() {
                                if owner == user {
                                    builder.set_inuse(capnp_rpc::new_client(machineapi.clone()));
                                }
                            }

                            MachineState::InUse
                        },
                        Status::Reserved(_) => MachineState::Reserved,
                        Status::ToCheck(_) => MachineState::ToCheck,
                    };
                    builder.set_state(s);

                    if perms.write && state == Status::Free {
                        builder.set_use(capnp_rpc::new_client(machineapi.clone()));
                    }
                    if perms.manage {
                        //builder.set_transfer(capnp_rpc::new_client(machineapi.clone()));
                        //builder.set_check(capnp_rpc::new_client(machineapi.clone()));
                        builder.set_manage(capnp_rpc::new_client(machineapi.clone()));
                    }
                    if permissions.iter().any(|r| r.match_perm(&admin_perm())) {
                        builder.set_admin(capnp_rpc::new_client(machineapi.clone()));
                    }

                    builder.set_info(capnp_rpc::new_client(machineapi));
                }

                Ok(())
            };

            Promise::from_future(f)
        } else {
            Promise::ok(())
        }
    }

    fn get_machine(&mut self,
        params: machines::GetMachineParams,
        mut results: machines::GetMachineResults
        ) -> Promise<(), capnp::Error> {
        let rc = Rc::clone(&self.session);
        if self.session.borrow().is_some() {
            let id = {
                let params = pry!(params.get());
                pry!(params.get_id()).to_string()
            };

            let network = self.network.clone();
            let f = async move {
                let session = rc.borrow();
                let user = &session.as_ref().unwrap().authzid;
                let permissions = &session.as_ref().unwrap().perms;

                if let Some(machine) = network.machines.get(&id) {
                    let mut builder = results.get().init_machine();
                    let perms = Perms::get_for(&machine.desc.privs, permissions.iter());
                    builder.set_id(&id);
                    builder.set_name(&machine.desc.name);
                    if let Some(ref desc) = machine.desc.description {
                        builder.set_description(desc);
                    }
                    if let Some(ref wiki) = machine.desc.wiki {
                        builder.set_wiki(wiki);
                    }
                    builder.set_urn(&format!("urn:fabaccess:resource:{}", &id));

                    let machineapi = Machine::new(user.clone(), perms, machine.clone());
                    let state = machine.get_status().await;
                    if perms.write && state == Status::Free {
                        builder.set_use(capnp_rpc::new_client(machineapi.clone()));
                    }
                    if perms.manage {
                        //builder.set_transfer(capnp_rpc::new_client(machineapi.clone()));
                        //builder.set_check(capnp_rpc::new_client(machineapi.clone()));
                        builder.set_manage(capnp_rpc::new_client(machineapi.clone()));
                    }
                    if permissions.iter().any(|r| r.match_perm(&admin_perm())) {
                        builder.set_admin(capnp_rpc::new_client(machineapi.clone()));
                    }


                    let s = match machine.get_status().await {
                        Status::Free => MachineState::Free,
                        Status::Disabled => MachineState::Disabled,
                        Status::Blocked(_) => MachineState::Blocked,
                        Status::InUse(u) => {
                            if let Some(owner) = u.as_ref() {
                                if owner == user {
                                    builder.set_inuse(capnp_rpc::new_client(machineapi.clone()));
                                }
                            }
                            MachineState::InUse
                        },
                        Status::Reserved(_) => MachineState::Reserved,
                        Status::ToCheck(_) => MachineState::ToCheck,
                    };
                    builder.set_state(s);

                    builder.set_info(capnp_rpc::new_client(machineapi));
                };

                Ok(())
            };
            Promise::from_future(f)
        } else {
            Promise::ok(())
        }
    }

    fn get_machine_u_r_n(
        &mut self,
        params: machines::GetMachineURNParams,
        mut results: machines::GetMachineURNResults
    ) -> Promise<(), capnp::Error> {
        let rc = Rc::clone(&self.session);
        if self.session.borrow().is_some() {
            let urn = {
                let params = pry!(params.get());
                pry!(params.get_urn())
            };

            let mut parts = urn.split_terminator(':');
            let part_urn = parts.next().map(|u| u == "urn").unwrap_or(false);
            let part_fabaccess = parts.next().map(|f| f == "fabaccess").unwrap_or(false);
            let part_resource = parts.next().map(|r| r == "resource").unwrap_or(false);
            if !(part_urn && part_fabaccess && part_resource) {
                return Promise::ok(())
            }

            if let Some(name) = parts.next() {
                let name = name.to_string();
                let network = self.network.clone();
                let f = async move {
                    let session = rc.borrow();
                    let user = &session.as_ref().unwrap().authzid;
                    let permissions = &session.as_ref().unwrap().perms;

                    if let Some(machine) = network.machines.get(&name) {
                        let mut builder = results.get().init_machine();
                        let perms = Perms::get_for(&machine.desc.privs, permissions.iter());
                        builder.set_name(&name);
                        if let Some(ref desc) = machine.desc.description {
                            builder.set_description(desc);
                        }
                        if let Some(ref wiki) = machine.desc.wiki {
                            builder.set_wiki(wiki);
                        }
                        builder.set_urn(&format!("urn:fabaccess:resource:{}", &name));

                        let machineapi = Machine::new(user.clone(), perms, machine.clone());
                        let state = machine.get_status().await;
                        if perms.write && state == Status::Free {
                            builder.set_use(capnp_rpc::new_client(machineapi.clone()));
                        }
                        if perms.manage {
                            //builder.set_transfer(capnp_rpc::new_client(machineapi.clone()));
                            //builder.set_check(capnp_rpc::new_client(machineapi.clone()));
                            builder.set_manage(capnp_rpc::new_client(machineapi.clone()));
                        }
                        if permissions.iter().any(|r| r.match_perm(&admin_perm())) {
                            builder.set_admin(capnp_rpc::new_client(machineapi.clone()));
                        }


                        let s = match machine.get_status().await {
                            Status::Free => MachineState::Free,
                            Status::Disabled => MachineState::Disabled,
                            Status::Blocked(_) => MachineState::Blocked,
                            Status::InUse(u) => {
                                if let Some(owner) = u.as_ref() {
                                    if owner == user {
                                        builder.set_inuse(capnp_rpc::new_client(machineapi.clone()));
                                    }
                                }
                                MachineState::InUse
                            },
                            Status::Reserved(_) => MachineState::Reserved,
                            Status::ToCheck(_) => MachineState::ToCheck,
                        };
                        builder.set_state(s);

                        builder.set_info(capnp_rpc::new_client(machineapi));
                    };

                    Ok(())
                };
                Promise::from_future(f)
            } else {
                Promise::ok(())
            }
        } else {
            Promise::ok(())
        }
    }
}
