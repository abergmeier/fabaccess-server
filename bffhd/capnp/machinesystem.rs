use crate::authorization::AuthorizationHandle;
use crate::session::SessionHandle;
use api::machinesystem_capnp::machine_system::{
    info, InfoParams, InfoResults, Server as MachineSystem,
};
use capnp::capability::Promise;

#[derive(Debug, Clone)]
pub struct Machines {
    session: SessionHandle,
}

impl Machines {
    pub fn new(session: SessionHandle) -> Self {
        Self { session }
    }
}

impl MachineSystem for Machines {
    fn info(&mut self, _: InfoParams, _: InfoResults) -> Promise<(), ::capnp::Error> {
        Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
}

impl info::Server for Machines {
    fn get_machine_list(
        &mut self,
        _: info::GetMachineListParams,
        _: info::GetMachineListResults,
    ) -> Promise<(), ::capnp::Error> {
        Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
    fn get_machine(
        &mut self,
        _: info::GetMachineParams,
        _: info::GetMachineResults,
    ) -> Promise<(), ::capnp::Error> {
        Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
    fn get_machine_u_r_n(
        &mut self,
        _: info::GetMachineURNParams,
        _: info::GetMachineURNResults,
    ) -> Promise<(), ::capnp::Error> {
        Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
}
