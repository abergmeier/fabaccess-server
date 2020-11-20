use std::sync::Arc;

use capnp::capability::Promise;
use capnp::Error;

use crate::schema::api_capnp::machine::*;
use crate::db::machine::MachineIdentifier;
use crate::connection::Session;
use crate::db::Databases;

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
}

struct Read(Arc<Machine>);

impl read::Server for Read {
    fn info(&mut self,
        _params: read::InfoParams,
        _results: read::InfoResults) 
    -> Promise<(), Error>
    {
        unimplemented!()
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
