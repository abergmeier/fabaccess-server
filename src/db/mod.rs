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
    pub roles: Box<dyn access::RoleDB>,
    pub user: Box<dyn user::UserDB>,
    pub machine: Box<dyn machine::MachineDB>,
}
