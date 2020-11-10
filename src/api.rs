use std::sync::Arc;

use slog::Logger;

use crate::error::Result;
pub use crate::schema::api_capnp;

use capnp::capability::Promise;
use capnp::Error;
use capnp_rpc::RpcSystem;
use capnp_rpc::twoparty::VatNetwork;
use capnp_rpc::rpc_twoparty_capnp::Side;
use capnp::capability::FromServer;

use crate::db::machine::Machines;
use crate::db::user::User;

use uuid::Uuid;

pub struct MachinesAPI {
    log: Logger,
    user: User,
    machines: Arc<Machines>,
}

impl MachinesAPI {
    pub fn new(log: Logger, user: User, machines: Arc<Machines>) -> Self {
        Self { log, user, machines }
    }
}

impl api_capnp::machines::Server for MachinesAPI {
    fn list_machines(&mut self,
        _params: api_capnp::machines::ListMachinesParams,
        mut results: api_capnp::machines::ListMachinesResults)
        -> Promise<(), Error>
    {
        let l = results.get();
        let keys: Vec<api_capnp::machine::Reader> = self.machines.iter().map(|x| x.into()).collect();
        l.set_machines(keys);
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
