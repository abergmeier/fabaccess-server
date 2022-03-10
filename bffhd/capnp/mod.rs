use std::future::Future;
use capnp::capability::Promise;
use capnp::Error;
use capnp_rpc::rpc_twoparty_capnp::Side;
use capnp_rpc::RpcSystem;
use capnp_rpc::twoparty::VatNetwork;
use futures_lite::StreamExt;
use futures_rustls::server::TlsStream;
use futures_util::{AsyncRead, AsyncWrite, FutureExt};

use crate::error::Result;

use api::bootstrap::{
    Client,
    Server,
    MechanismsParams,
    MechanismsResults,
    CreateSessionParams,
    CreateSessionResults
};

mod authenticationsystem;
mod machine;
mod machinesystem;
mod permissionsystem;
mod user;
mod users;
mod session;

#[derive(Debug)]
pub struct APIHandler {

}

impl APIHandler {
    pub fn handle<IO: 'static + Unpin + AsyncRead + AsyncWrite>(&mut self, stream: TlsStream<IO>)
        -> impl Future<Output = Result<()>>
    {
        let (rx, tx) = futures_lite::io::split(stream);
        let vat = VatNetwork::new(rx, tx, Side::Server, Default::default());
        let bootstrap: Client = capnp_rpc::new_client(ApiSystem::new());

        RpcSystem::new(Box::new(vat), Some(bootstrap.client))
            .map(|res| match res {
                Ok(()) => Ok(()),
                Err(e) => Err(e.into())
            })
    }
}

#[derive(Debug)]
/// Cap'n Proto API Handler
struct ApiSystem {

}

impl ApiSystem {
    pub fn new() -> Self {
        Self {}
    }
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