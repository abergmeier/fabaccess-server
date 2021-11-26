use capnp::capability::Promise;
use capnp::Error;

use api::bootstrap::{
    Server,
    MechanismsParams,
    MechanismsResults,
    CreateSessionParams,
    CreateSessionResults
};

mod tls;
mod authentication;

struct ApiSystem {

}

impl Server for ApiSystem {
    fn mechanisms(
        &mut self,
        _: MechanismsParams,
        _: MechanismsResults
    ) -> Promise<(), Error>
    {
        todo!()
    }

    fn create_session(
        &mut self,
        _: CreateSessionParams,
        _: CreateSessionResults
    ) -> Promise<(), Error>
    {
        todo!()
    }
}