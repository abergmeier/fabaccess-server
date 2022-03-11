use api::connection_capnp::bootstrap::Server as Bootstrap;
pub use api::connection_capnp::bootstrap::Client;

#[derive(Debug)]
/// Cap'n Proto API Handler
pub struct BootCap;

impl BootCap {
    pub fn new() -> Self {
        Self
    }
}

impl Bootstrap for BootCap {
}