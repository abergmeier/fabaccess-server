use smol::net::TcpStream;
use futures_util::FutureExt;

use slog::Logger;

use crate::error::Result;
pub use crate::schema::api_capnp;

use capnp::capability::Promise;
use capnp::Error;
use capnp_rpc::RpcSystem;
use capnp_rpc::twoparty::VatNetwork;
use capnp_rpc::rpc_twoparty_capnp::Side;
use capnp::capability::FromServer;

pub async fn handle_connection(log: Logger, socket: TcpStream) -> Result<()> {
    let mut message = capnp::message::Builder::new_default();
    let mut outer = message.init_root::<crate::connection::connection_capnp::message::Builder>();
    let mut api = outer.init_api();

    let mapi = MachinesAPI {};
    api.set_machines(capnp_rpc::new_client(mapi));

    let network = VatNetwork::new(socket.clone(), socket, Side::Server, Default::default());
    let rpc = RpcSystem::new(Box::new(network), None).map(|_| ());

    rpc.await;

    Ok(())
}

pub struct MachinesAPI;

impl api_capnp::machines::Server for MachinesAPI {
    fn list_machines(&mut self,
        _params: api_capnp::machines::ListMachinesParams,
        mut results: api_capnp::machines::ListMachinesResults)
        -> Promise<(), Error>
    {
        let mut l = results.get();
        l.init_machines(0);
        Promise::ok(())
    }

    fn get_machine(&mut self,
        _params: api_capnp::machines::GetMachineParams,
        mut results: api_capnp::machines::GetMachineResults)
        -> Promise<(), Error>
    {
        Promise::ok(())
    }
}
