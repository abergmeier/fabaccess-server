// module needs to be top level for generated functions to be in scope:
// https://github.com/capnproto/capnproto-rust/issues/16
pub mod gen {
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
    let api = gen::diflouroborane::ToClient::new(client).into_client::<capnp_rpc::Server>();

    let mut message = capnp::message::Builder::new_default();
    let mut outer = message.init_root::<crate::connection::gen::message::Builder>();
    outer.set_api(api.clone());

    let network = VatNetwork::new(socket.clone(), socket, Side::Server, Default::default());
    let rpc = RpcSystem::new(Box::new(network), Some(api.client)).map(|_| ());

    rpc.await;

    Ok(())
}

pub struct DifAPI;

impl gen::diflouroborane::Server for DifAPI {
    fn machines(&mut self,
        _params: gen::diflouroborane::MachinesParams,
        mut results: gen::diflouroborane::MachinesResults)
        -> Promise<(), Error>
    {
        let mut b = results.get();
        let mach = gen::machines::ToClient::new(MachinesAPI).into_client::<capnp_rpc::Server>();
        b.set_mach(mach);
        Promise::ok(())
    }
}

pub struct MachinesAPI;

impl gen::machines::Server for MachinesAPI {
    fn list(&mut self,
        _params: gen::machines::ListParams,
        mut results: gen::machines::ListResults)
        -> Promise<(), Error>
    {
        let mut l = results.get();
        l.init_machines(0);
        Promise::ok(())
    }
}
