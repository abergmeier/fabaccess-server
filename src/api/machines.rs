use std::sync::Arc;

use capnp::capability::Promise;
use capnp::Error;

use crate::schema::machinesystem_capnp::machine_system;
use crate::schema::machinesystem_capnp::machine_system::info as machines;
use crate::connection::Session;

use crate::db::Databases;

use crate::network::Network;

use super::machine::*;

/// An implementation of the `Machines` API
pub struct Machines {
    /// A reference to the connection — as long as at least one API endpoint is
    /// still alive the session has to be as well.
    session: Arc<Session>,

    db: Databases,
    network: Arc<Network>,
}

impl Machines {
    pub fn new(session: Arc<Session>, db: Databases, network: Arc<Network>) -> Self {
        info!(session.log, "Machines created");
        Self { session, db, network }
    }
}

impl machine_system::Server for Machines {

}

impl machines::Server for Machines {
    fn get_machine_list(&mut self,
        _params: machines::GetMachineListParams,
        mut results: machines::GetMachineListResults)
        -> Promise<(), Error>
    {
        let v: Vec<(String, crate::machine::Machine)> = self.network.machines.iter()
            .map(|(n, m)| (n.clone(), m.clone()))
            .collect();

        /*let mut machines = results.get().init_machines(v.len() as u32);

        for (i, (name, machine)) in v.into_iter().enumerate() {
            trace!(self.session.log, "Adding machine #{} {}: {:?}", i, name, machine);
            let machine = Arc::new(Machine::new(self.session.clone(), machine, self.db.clone()));
            let mut builder = machines.reborrow().get(i as u32);
            machine.fill(&mut builder);
        }*/

        Promise::ok(())
    }
}
