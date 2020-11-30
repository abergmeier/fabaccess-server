use std::sync::Arc;

/// (Hashed) password database
pub mod pass;

/// User storage
pub mod user;

/// Access control storage
///
/// Stores&Retrieves Permissions and Roles
pub mod access;

/// Machine storage
///
/// Stores&Retrieves Machines
pub mod machine;

#[derive(Clone)]
pub struct Databases {
    pub access: Arc<access::AccessControl>,
    pub machine: Arc<machine::MachineDB>,
    pub passdb: Arc<pass::PassDB>,
}
