use std::sync::Arc;


/// Access control storage
///
/// Stores&Retrieves Permissions and Roles
pub mod access;
/// User storage
///
/// Stores&Retrieves Users
pub mod user;

/// Machine storage
///
/// Stores&Retrieves Machines
pub mod machine;

#[derive(Clone)]
pub struct Databases {
    pub access: Arc<access::AccessControl>,
    pub machine: Arc<machine::MachineDB>,
}
