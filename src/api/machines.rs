use std::sync::Arc;

use capnp::capability::Promise;
use capnp::Error;

use crate::schema::api_capnp::machines;
use crate::connection::Session;

use crate::db::Databases;

use crate::network::Network;

use super::machine::Machine;

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

impl machines::Server for Machines {
    fn list_machines(&mut self,
        _params: machines::ListMachinesParams,
        mut results: machines::ListMachinesResults)
        -> Promise<(), Error>
    {
        let v: Vec<(String, crate::machine::Machine)> = self.network.machines.iter()
            .map(|(n, m)| (n.clone(), m.clone()))
            .collect();

        let mut res = results.get();
        let mut machines = res.init_machines(v.len() as u32);

        for (i, (name, machine)) in v.into_iter().enumerate() {
            let machine = Arc::new(Machine::new(self.session.clone(), machine, self.db.clone()));
            let mut builder = machines.reborrow().get(i as u32);
            Machine::fill(machine, &mut builder);
        }

        Promise::ok(())
    }

    fn get_machine(&mut self,
        params: machines::GetMachineParams,
        mut results: machines::GetMachineResults)
        -> Promise<(), Error>
    {
        if let Ok(uid) = params.get().and_then(|x| x.get_uid()) {
            if let Some(machine_inner) = self.network.machines.get(uid) {
                let machine = Arc::new(Machine::new(self.session.clone(), machine_inner.clone(), self.db.clone()));
                let mut builder = results.get().init_machine();
                Machine::fill(machine, &mut builder);
            }
        }

        Promise::ok(())
    }
}
