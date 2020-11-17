use crate::schema::api_capnp::machines;

use capnp::capability::Promise;
use capnp::Error;

struct Machines;

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
