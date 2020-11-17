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

pub struct Databases {
    pub access: access::internal::Internal,
    pub machine: machine::internal::Internal,
}
