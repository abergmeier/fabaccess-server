use std::sync::Arc;

use capnp::capability::Promise;
use capnp::Error;

use crate::schema::api_capnp::State;
use crate::schema::api_capnp::machine::*;
use crate::db::machine::MachineIdentifier;
use crate::connection::Session;
use crate::db::Databases;
use crate::db::machine::Status;

#[derive(Clone)]
pub struct Machine {
    session: Arc<Session>,
    id: MachineIdentifier,
    db: Databases,
}

impl Machine {
    pub fn new(session: Arc<Session>, id: MachineIdentifier, db: Databases) -> Self {
        Machine { session, id, db }
    }

    pub fn fill(self: Arc<Self>, builder: &mut Builder) {
        // TODO check permissions
        builder.set_read(capnp_rpc::new_client(Read(self.clone())));
        // TODO set all the others
    }

    pub fn fill_info(&self, builder: &mut m_info::Builder) {
        if let Some(desc) = self.db.machine.get_desc(&self.id) {
            builder.set_name(&desc.name);
            if let Some(d) = desc.description.as_ref() {
                builder.set_description(d);
            }

            // TODO: Set `responsible`
            // TODO: Error Handling
            if let Some(state) = self.db.machine.get_state(&self.id) {
                match state.state {
                    Status::Free => builder.set_state(State::Free),
                    Status::InUse(_u) => {
                        builder.set_state(State::InUse);
                    }
                    Status::ToCheck(_u) => {
                        builder.set_state(State::ToCheck);
                    }
                    Status::Blocked(_u) => {
                        builder.set_state(State::Blocked);
                    }
                    Status::Disabled => builder.set_state(State::Disabled),
                    Status::Reserved(_u) => {
                        builder.set_state(State::Reserved);
                    }
                }
            }
        }
    }
}

struct Read(Arc<Machine>);

impl read::Server for Read {
    fn info(&mut self,
        _params: read::InfoParams,
        mut results: read::InfoResults) 
    -> Promise<(), Error>
    {
        let mut b = results.get().init_minfo();
        self.0.fill_info(&mut b);
        Promise::ok(())
    }
}

struct Write(Arc<Machine>);

impl write::Server for Write {
    fn use_(&mut self,
        _params: write::UseParams,
        _results: write::UseResults)
    -> Promise<(), Error>
    {
        unimplemented!()
    }
}

struct Manage(Arc<Machine>);

impl manage::Server for Manage {
    fn ok(&mut self,
        _params: manage::OkParams,
        _results: manage::OkResults)
    -> Promise<(), Error>
    {
        unimplemented!()
    }
}

struct Admin(Arc<Machine>);

impl admin::Server for Admin {
    fn force_set_state(&mut self,
        _params: admin::ForceSetStateParams,
        _results: admin::ForceSetStateResults)
    -> Promise<(), Error>
    {
        unimplemented!()
    }

    fn force_set_user(&mut self,
        _params: admin::ForceSetUserParams,
        _results: admin::ForceSetUserResults)
    -> Promise<(), Error>
    {
        unimplemented!()
    }
}
