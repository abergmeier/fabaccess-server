use api::session::Builder;
use crate::capnp::machinesystem::Resources;
use crate::capnp::users::Users;

#[derive(Debug, Clone)]
pub struct Session {
    resources: Resources,
    users: Users,
}

impl Session {
    pub fn new() -> Self {
        Session {
            resources: Resources::new(),
            users: Users::new(),
        }
    }

    pub fn build(&self, builder: &mut Builder) {
        builder.set_resources(capnp_rpc::new_client(self.resources.clone()));
        builder.set_users(capnp_rpc::new_client(self.users.clone()));
    }
}