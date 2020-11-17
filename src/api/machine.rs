use crate::schema::api_capnp::machine::*;

use capnp::capability::Promise;
use capnp::Error;


struct Machine;

impl read::Server for Machine {
    fn info(&mut self,
        _params: read::InfoParams,
        _results: read::InfoResults) 
    -> Promise<(), Error>
    {
        unimplemented!()
    }
}

impl write::Server for Machine {
    fn use_(&mut self,
        _params: write::UseParams,
        _results: write::UseResults)
    -> Promise<(), Error>
    {
        unimplemented!()
    }
}

impl manage::Server for Machine {
    fn ok(&mut self,
        _params: manage::OkParams,
        _results: manage::OkResults)
    -> Promise<(), Error>
    {
        unimplemented!()
    }
}

impl admin::Server for Machine {
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
