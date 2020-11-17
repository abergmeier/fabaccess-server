use std::sync::Arc;

use capnp::capability::Promise;
use capnp::Error;

use crate::schema::api_capnp::machines;
use crate::connection::Session;

/// An implementation of the `Machines` API
struct Machines {
    /// A reference to the connection — as long as at least one API endpoint is
    /// still alive the session has to be as well.
    session: Arc<Session>,
}

impl Machines {
    pub fn new(session: Arc<Session>) -> Self {
        Self { session }
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
        _params: machines::GetMachineParams,
        mut results: machines::GetMachineResults)
        -> Promise<(), Error>
    {
        Promise::ok(())
    }
}
