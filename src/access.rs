//! Access control logic
//!

use std::collections::HashSet;

use flexbuffers;
use serde::{Serialize, Deserialize};

use slog::Logger;
use lmdb::{Transaction, RoTransaction, RwTransaction};

use crate::config::Config;
use crate::error::Result;

type UserIdentifier = u64;
type RoleIdentifier = u64;
type PermIdentifier = u64;

pub struct PermissionsProvider {
    log: Logger,
    roledb: lmdb::Database,
    permdb: lmdb::Database,
    userdb: lmdb::Database,
}

impl PermissionsProvider {
    pub fn new(log: Logger, roledb: lmdb::Database, permdb: lmdb::Database, userdb: lmdb::Database) -> Self {
        Self { log, roledb, permdb, userdb }
    }

    /// Check if a given user has the given permission
    #[allow(unused)]
    pub fn check<T: Transaction>(&self, txn: &T, userID: UserIdentifier, permID: PermIdentifier) -> Result<bool> {
        if let Some(user) = self.get_user(txn, userID)? {
            // Tally all roles. Makes dependent roles easier
            let mut roles = HashSet::new();
            for roleID in user.roles {
                self.tally_role(txn, &mut roles, roleID)?;
            }

            // Iter all unique role->permissions we've found and early return on match. 
            // TODO: Change this for negative permissions?
            for role in roles.iter() {
                for perm in role.permissions.iter() {
                    if permID == *perm {
                        return Ok(true);
                    }
                }
            }
        }

        return Ok(false);
    }

    fn tally_role<T: Transaction>(&self, txn: &T, roles: &mut HashSet<Role>, roleID: RoleIdentifier) -> Result<()> {
        if let Some(role) = self.get_role(txn, roleID)? {
            // Only check and tally parents of a role at the role itself if it's the first time we
            // see it
            if !roles.contains(&role) {
                for parent in role.parents.iter() {
                    self.tally_role(txn, roles, *parent)?;
                }

                roles.insert(role);
            }
        }

        Ok(())
    }

    fn get_role<'txn, T: Transaction>(&self, txn: &'txn T, roleID: RoleIdentifier) -> Result<Option<Role>> {
        match txn.get(self.roledb, &roleID.to_ne_bytes()) {
            Ok(bytes) => {
                Ok(Some(flexbuffers::from_slice(bytes)?))
            },
            Err(lmdb::Error::NotFound) => { Ok(None) },
            Err(e) => { Err(e.into()) }
        }
    }

    fn get_user<T: Transaction>(&self, txn: &T, userID: UserIdentifier) -> Result<Option<User>> {
        match txn.get(self.userdb, &userID.to_ne_bytes()) {
            Ok(bytes) => {
                Ok(Some(flexbuffers::from_slice(bytes)?))
            },
            Err(lmdb::Error::NotFound) => { Ok(None) },
            Err(e) => { Err(e.into()) }
        }
    }

    fn get_perm<T: Transaction>(&self, txn: &T, permID: PermIdentifier) -> Result<Option<Perm>> {
        match txn.get(self.permdb, &permID.to_ne_bytes()) {
            Ok(bytes) => {
                Ok(Some(flexbuffers::from_slice(bytes)?))
            },
            Err(lmdb::Error::NotFound) => { Ok(None) },
            Err(e) => { Err(e.into()) }
        }
    }

   fn put_role(&self, txn: &mut RwTransaction, roleID: RoleIdentifier, role: Role) -> Result<()> {
       let bytes = flexbuffers::to_vec(role)?;
       txn.put(self.roledb, &roleID.to_ne_bytes(), &bytes, lmdb::WriteFlags::empty())?;

       Ok(())
   }

   fn put_user(&self, txn: &mut RwTransaction, userID: UserIdentifier, user: User) -> Result<()> {
       let bytes = flexbuffers::to_vec(user)?;
       txn.put(self.userdb, &userID.to_ne_bytes(), &bytes, lmdb::WriteFlags::empty())?;

       Ok(())
   }

   fn put_perm(&self, txn: &mut RwTransaction, permID: PermIdentifier, perm: Perm) -> Result<()> {
       let bytes = flexbuffers::to_vec(perm)?;
       txn.put(self.permdb, &permID.to_ne_bytes(), &bytes, lmdb::WriteFlags::empty())?;

       Ok(())
   }
}

/// This line documents init
pub fn init(log: Logger, config: &Config, env: &lmdb::Environment) -> std::result::Result<PermissionsProvider, crate::error::Error> {
    let mut flags = lmdb::DatabaseFlags::empty();
    flags.set(lmdb::DatabaseFlags::INTEGER_KEY, true);
    let roledb = env.create_db(Some("role"), flags)?;
    debug!(&log, "Opened access database '{}' successfully.", "role");
    let permdb = env.create_db(Some("perm"), flags)?;
    debug!(&log, "Opened access database '{}' successfully.", "perm");
    let userdb = env.create_db(Some("user"), flags)?;
    debug!(&log, "Opened access database '{}' successfully.", "user");
    info!(&log, "Opened all access databases");
    return Ok(PermissionsProvider::new(log, roledb, permdb, userdb));
}

/// A Person, from the Authorization perspective
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct User {
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
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct Perm {
    name: String,
}
