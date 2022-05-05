use crate::capnp::machine::Machine;
use crate::resources::search::ResourcesHandle;
use crate::resources::Resource;
use crate::session::SessionHandle;
use crate::RESOURCES;
use api::machinesystem_capnp::machine_system::info;
use capnp::capability::Promise;
use capnp_rpc::pry;

#[derive(Clone)]
pub struct Machines {
    session: SessionHandle,
    resources: ResourcesHandle,
}

impl Machines {
    pub fn new(session: SessionHandle) -> Self {
        // FIXME no unwrap bad
        Self {
            session,
            resources: RESOURCES.get().unwrap().clone(),
        }
    }
}

impl info::Server for Machines {
    fn get_machine_list(
        &mut self,
        _: info::GetMachineListParams,
        mut result: info::GetMachineListResults,
    ) -> Promise<(), ::capnp::Error> {
        let machine_list: Vec<(usize, &Resource)> = self
            .resources
            .list_all()
            .into_iter()
            .filter(|resource| resource.visible(&self.session))
            .enumerate()
            .collect();
        let mut builder = result.get().init_machine_list(machine_list.len() as u32);
        for (i, m) in machine_list {
            let resource = m.clone();
            let mbuilder = builder.reborrow().get(i as u32);
            Machine::build(self.session.clone(), resource, mbuilder);
        }

        Promise::ok(())
    }

    fn get_machine(
        &mut self,
        params: info::GetMachineParams,
        mut result: info::GetMachineResults,
    ) -> Promise<(), ::capnp::Error> {
        let params = pry!(params.get());
        let id = pry!(params.get_id());

        if let Some(resource) = self.resources.get_by_id(id) {
            let builder = result.get();
            Machine::optional_build(self.session.clone(), resource.clone(), builder);
        }

        Promise::ok(())
    }

    fn get_machine_u_r_n(
        &mut self,
        params: info::GetMachineURNParams,
        mut result: info::GetMachineURNResults,
    ) -> Promise<(), ::capnp::Error> {
        let params = pry!(params.get());
        let urn = pry!(params.get_urn());

        if let Some(resource) = self.resources.get_by_urn(urn) {
            let builder = result.get();
            Machine::optional_build(self.session.clone(), resource.clone(), builder);
        }

        Promise::ok(())
    }
}
