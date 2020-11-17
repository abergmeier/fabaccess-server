use crate::schema::api_capnp::machine::*;

use capnp::capability::Promise;
use capnp::Error;


struct Machine;

impl Machine {
    pub fn new() -> Self {
        Machine
    }
}

struct Read;

impl read::Server for Read {
    fn info(&mut self,
        _params: read::InfoParams,
        _results: read::InfoResults) 
    -> Promise<(), Error>
    {
        unimplemented!()
    }
}

struct Write;

impl write::Server for Write {
    fn use_(&mut self,
        _params: write::UseParams,
        _results: write::UseResults)
    -> Promise<(), Error>
    {
        unimplemented!()
    }
}

struct Manage;

impl manage::Server for Manage {
    fn ok(&mut self,
        _params: manage::OkParams,
        _results: manage::OkResults)
    -> Promise<(), Error>
    {
        unimplemented!()
    }
}

struct Admin;

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
