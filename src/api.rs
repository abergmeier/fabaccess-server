// module needs to be top level for generated functions to be in scope:
// https://github.com/capnproto/capnproto-rust/issues/16
pub mod api_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/api_capnp.rs"));
}

use smol::net::TcpStream;
use futures_util::FutureExt;

use slog::Logger;

use crate::error::Result;

use capnp::capability::Promise;
use capnp::Error;
use capnp_rpc::RpcSystem;
use capnp_rpc::twoparty::VatNetwork;
use capnp_rpc::rpc_twoparty_capnp::Side;

pub async fn handle_connection(log: Logger, socket: TcpStream) -> Result<()> {
    let client = DifAPI {};
    let api: api_capnp::diflouroborane::Client = capnp_rpc::new_client(client);

    let mut message = capnp::message::Builder::new_default();
    let mut outer = message.init_root::<crate::connection::connection_capnp::message::Builder>();
    outer.set_api(api.clone());

    let network = VatNetwork::new(socket.clone(), socket, Side::Server, Default::default());
    let rpc = RpcSystem::new(Box::new(network), Some(api.client)).map(|_| ());

    rpc.await;

    Ok(())
}

pub struct DifAPI;

impl api_capnp::diflouroborane::Server for DifAPI {
    fn machines(&mut self,
        _params: api_capnp::diflouroborane::MachinesParams,
        mut results: api_capnp::diflouroborane::MachinesResults)
        -> Promise<(), Error>
    {
        let mut b = results.get();
        let mach = capnp_rpc::new_client(MachinesAPI);
        b.set_mach(mach);
        Promise::ok(())
    }
}

pub struct MachinesAPI;

impl api_capnp::machines::Server for MachinesAPI {
    fn list(&mut self,
        _params: api_capnp::machines::ListParams,
        mut results: api_capnp::machines::ListResults)
        -> Promise<(), Error>
    {
        let mut l = results.get();
        l.init_machines(0);
        Promise::ok(())
    }
}
