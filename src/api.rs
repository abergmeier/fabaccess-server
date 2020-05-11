// module needs to be top level for generated functions to be in scope:
// https://github.com/capnproto/capnproto-rust/issues/16
pub mod gen {
    include!(concat!(env!("OUT_DIR"), "/schema/api_capnp.rs"));
}

use async_std::net::TcpStream;
use futures::io::{AsyncRead, AsyncWrite};

use slog::Logger;

use crate::error::Result;

pub async fn handle_connection(log: Logger, socket: TcpStream) -> Result<()> {
    unimplemented!()
}
