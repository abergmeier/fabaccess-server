use std::future::Future;
use futures_util::future::FutureExt;
use async_rustls::TlsStream;
use capnp::capability::Promise;
use capnp::Error;
use capnp_rpc::rpc_twoparty_capnp::Side;
use capnp_rpc::RpcSystem;
use capnp_rpc::twoparty::VatNetwork;
use smol::io::{AsyncRead, AsyncWrite};

use crate::error::Result;

use api::bootstrap::{
    Client,
    Server,
    MechanismsParams,
    MechanismsResults,
    CreateSessionParams,
    CreateSessionResults
};

mod authentication;
mod session;
mod users;
mod resources;

#[derive(Debug)]
pub struct APIHandler {

}

impl APIHandler {
    pub fn handle<IO: 'static + Unpin + AsyncRead + AsyncWrite>(&mut self, stream: TlsStream<IO>)
        -> impl Future<Output = Result<()>>
    {
        let (mut reader, mut writer) = smol::io::split(stream);

        let bootstrap = ApiSystem {};
        let rpc: Client = capnp_rpc::new_client(bootstrap);
        let network = VatNetwork::new(
            reader,
            writer,
            Side::Server,
            Default::default(),
        );
        let rpc_system = RpcSystem::new(Box::new(network), Some(rpc.client));

        rpc_system.map(|r| r.map_err(Into::into))
    }
}

#[derive(Debug)]
/// Cap'n Proto API Handler
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