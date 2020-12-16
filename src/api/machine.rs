use std::sync::Arc;

use capnp::capability::Promise;
use capnp::Error;

use crate::schema::api_capnp::State;
use crate::schema::api_capnp::machine::*;
use crate::connection::Session;
use crate::db::Databases;
use crate::db::machine::Status;
use crate::machine::Machine as NwMachine;

#[derive(Clone)]
pub struct Machine {
    session: Arc<Session>,
    machine: NwMachine,
    db: Databases,
}

impl Machine {
    pub fn new(session: Arc<Session>, machine: NwMachine, db: Databases) -> Self {
        Machine { session, machine, db }
    }

    pub fn fill(self: Arc<Self>, builder: &mut Builder) {
        // TODO check permissions
        builder.set_read(capnp_rpc::new_client(Read(self.clone())));
        // TODO set all the others
    }

    pub fn fill_info(&self, builder: &mut m_info::Builder) {
        unimplemented!()
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
