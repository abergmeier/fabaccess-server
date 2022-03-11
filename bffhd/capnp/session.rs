use crate::capnp::machinesystem::Machines;
use crate::capnp::user_system::Users;

#[derive(Debug, Clone)]
pub struct Session {
    resources: Machines,
    users: Users,
}

impl Session {
    pub fn new() -> Self {
        Session {
            resources: Machines::new(),
            users: Users::new(),
        }
    }
}