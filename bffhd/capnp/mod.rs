use std::future::Future;
use capnp::capability::Promise;
use capnp::Error;
use futures_rustls::server::TlsStream;
use futures_util::{AsyncRead, AsyncWrite};

use crate::error::Result;

use api::bootstrap::{
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
        unimplemented!();
        futures_util::future::ready(Ok(()))
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