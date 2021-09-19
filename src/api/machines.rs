use std::sync::Arc;

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

/// An implementation of the `Machines` API
#[derive(Clone)]
pub struct Machines {
    user: UserId,
    permissions: Vec<PermRule>,
    network: Arc<Network>,
}

impl Machines {
    pub fn new(user: UserId, permissions: Vec<PermRule>, network: Arc<Network>) -> Self {
        Self { user, permissions, network }
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
        let v: Vec<(String, crate::machine::Machine)> = self.network.machines.iter()
            .filter(|(_name, machine)| {
                let required_disclose = &machine.desc.privs.disclose;
                for perm_rule in self.permissions.iter() {
                    if perm_rule.match_perm(required_disclose) {
                        return true;
                    }
                }

                false
            })
            .map(|(n,m)| (n.clone(), m.clone()))
            .collect();

        let permissions = self.permissions.clone();
        let user = self.user.clone();

        let f = async move {
            let mut machines = results.get().init_machine_list(v.len() as u32);
            for (i, (name, machine)) in v.into_iter().enumerate() {
                let perms = Perms::get_for(&machine.desc.privs, permissions.iter());

                let mut builder = machines.reborrow().get(i as u32);
                builder.set_name(&name);
                if let Some(ref desc) = machine.desc.description {
                    builder.set_description(desc);
                }

                let s = match machine.get_status().await {
                    Status::Free => MachineState::Free,
                    Status::Disabled => MachineState::Disabled,
                    Status::Blocked(_) => MachineState::Blocked,
                    Status::InUse(_) => MachineState::InUse,
                    Status::Reserved(_) => MachineState::Reserved,
                    Status::ToCheck(_) => MachineState::ToCheck,
                };
                builder.set_state(s);

                let machineapi = Machine::new(user.clone(), perms, machine.clone());
                if perms.write {
                    builder.set_use(capnp_rpc::new_client(machineapi.clone()));
                    builder.set_inuse(capnp_rpc::new_client(machineapi.clone()));
                }
                if perms.manage {
                    builder.set_transfer(capnp_rpc::new_client(machineapi.clone()));
                    builder.set_check(capnp_rpc::new_client(machineapi.clone()));
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
    }

    fn get_machine(&mut self,
        params: machines::GetMachineParams,
        mut results: machines::GetMachineResults
        ) -> Promise<(), capnp::Error> {

        let name = {
            let params = pry!(params.get());
            pry!(params.get_name()).to_string()
        };

        let network = self.network.clone();
        let user = self.user.clone();
        let permissions = self.permissions.clone();

        let f = async move {
            if let Some(machine) = network.machines.get(&name) {
                let mut builder = results.get().init_machine();
                let perms = Perms::get_for(&machine.desc.privs, permissions.iter());
                builder.set_name(&name);
                if let Some(ref desc) = machine.desc.description {
                    builder.set_description(desc);
                }

                let machineapi = Machine::new(user.clone(), perms, machine.clone());
                if perms.write {
                    builder.set_use(capnp_rpc::new_client(machineapi.clone()));
                    builder.set_inuse(capnp_rpc::new_client(machineapi.clone()));
                }
                if perms.manage {
                    builder.set_transfer(capnp_rpc::new_client(machineapi.clone()));
                    builder.set_check(capnp_rpc::new_client(machineapi.clone()));
                    builder.set_manage(capnp_rpc::new_client(machineapi.clone()));
                }
                if permissions.iter().any(|r| r.match_perm(&admin_perm())) {
                    builder.set_admin(capnp_rpc::new_client(machineapi.clone()));
                }

                builder.set_info(capnp_rpc::new_client(machineapi));

                let s = match machine.get_status().await {
                    Status::Free => MachineState::Free,
                    Status::Disabled => MachineState::Disabled,
                    Status::Blocked(_) => MachineState::Blocked,
                    Status::InUse(_) => MachineState::InUse,
                    Status::Reserved(_) => MachineState::Reserved,
                    Status::ToCheck(_) => MachineState::ToCheck,
                };
                builder.set_state(s);
            };

            Ok(())
        };
        Promise::from_future(f)
    }
}
