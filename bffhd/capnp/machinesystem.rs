use api::machinesystem_capnp::machine_system::Server as MachineSystem;

#[derive(Debug, Clone)]
pub struct Machines {

}

impl Machines {
    pub fn new() -> Self {
        Self {

        }
    }

}

impl MachineSystem for Machines {

}