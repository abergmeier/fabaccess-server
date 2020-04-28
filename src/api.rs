// module needs to be top level for generated functions to be in scope:
// https://github.com/capnproto/capnproto-rust/issues/16
pub mod api {
    include!(concat!(env!("OUT_DIR"), "/schema/api_capnp.rs"));
}

use std::default::Default;
use async_std::net::TcpStream;

use futures::task::Spawn;
use futures::FutureExt;
use futures_signals::signal::Mutable;
use casbin::Enforcer;
use casbin::MgmtApi;

use slog::Logger;

use std::rc::Rc;
use async_std::sync::{Arc, RwLock};

use crate::machine::{MachinesProvider, Machines};
use crate::auth::{AuthenticationProvider, Authentication};
use crate::access::{PermissionsProvider, Permissions};

use capnp::{Error};
use capnp::capability::Promise;
use capnp_rpc::RpcSystem;
use capnp_rpc::twoparty::VatNetwork;
use capnp_rpc::rpc_twoparty_capnp::Side;

use std::ops::Deref;

use api::diflouroborane;

pub async fn handle_connection(log: Logger, socket: TcpStream) -> Result<(), Error> {
    unimplemented!()
}
