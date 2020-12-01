use std::sync::Arc;

use capnp::capability::Promise;
use capnp::Error;

use crate::schema::api_capnp::machines;
use crate::connection::Session;

use crate::db::Databases;
use crate::db::machine::uuid_from_api;

use super::machine::Machine;

/// An implementation of the `Machines` API
pub struct Machines {
    /// A reference to the connection — as long as at least one API endpoint is
    /// still alive the session has to be as well.
    session: Arc<Session>,

    db: Databases,
}

impl Machines {
    pub fn new(session: Arc<Session>, db: Databases) -> Self {
        info!(session.log, "Machines created");
        Self { session, db }
    }
}

impl machines::Server for Machines {
    fn list_machines(&mut self,
        _params: machines::ListMachinesParams,
        mut results: machines::ListMachinesResults)
        -> Promise<(), Error>
    {
        Promise::ok(())
    }

    fn get_machine(&mut self,
        params: machines::GetMachineParams,
        mut results: machines::GetMachineResults)
        -> Promise<(), Error>
    {
        unimplemented!()
    }
}
