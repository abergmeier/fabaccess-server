//! Access control logic
//!

use slog::Logger;

use crate::config::Config;


pub struct PermissionsProvider {
    log: Logger,
}

impl PermissionsProvider {
    pub fn new(log: Logger) -> Self {
        Self { log }
    }
}

/// This line documents init
pub fn init(log: Logger, config: &Config, env: &lmdb::Environment) -> std::result::Result<PermissionsProvider, crate::error::Error> {
    return Ok(PermissionsProvider::new(log));
}

type RoleIdentifier = u64;
type PermIdentifier = u64;

/// A Person, from the Authorization perspective
struct Person {
    name: String,

    /// A Person has N ≥ 0 roles.
    /// Persons are only ever given roles, not permissions directly
    roles: Vec<RoleIdentifier>
}

/// A "Role" from the Authorization perspective
///
/// You can think of a role as a bundle of permissions relating to other roles. In most cases a
/// role represents a real-world education or apprenticeship, which gives a person the education
/// necessary to use a machine safely.
/// Roles are assigned permissions which in most cases evaluate to granting a person the right to
/// use certain (potentially) dangerous machines. 
/// Using this indirection makes administration easier in certain ways; instead of maintaining
/// permissions on users directly the user is given a role after having been educated on the safety
/// of a machine; if later on a similar enough machine is put to use the administrator can just add
/// the permission for that machine to an already existing role instead of manually having to
/// assign to all users.
struct Role {
    name: String,

    /// A Role can have parents, inheriting all permissions
    ///
    /// This makes situations where different levels of access are required easier: Each higher
    /// level of access sets the lower levels of access as parent, inheriting their permission; if
    /// you are allowed to manage a machine you are then also allowed to use it and so on
    parents: Vec<RoleIdentifier>,
    permissions: Vec<PermIdentifier>,
}

/// A Permission from the Authorization perspective
///
/// Permissions are rather simple flags. A person can have or not have a permission, dictated by
/// its roles and the permissions assigned to those roles.
struct Permission {
    name: String,
}
