//! Access control logic
//!

use std::fmt;
use std::collections::HashSet;

use std::convert::TryInto;

use std::path::{Path, PathBuf};
use std::fs;
use std::io::Write;
use std::sync::Arc;

use flexbuffers;
use serde::{Serialize, Deserialize};

use slog::Logger;
use lmdb::{Environment, Transaction, RwTransaction, Cursor};

use crate::config::Settings;
use crate::error::Result;

mod adapter_lmdb;

use adapter_lmdb::PermissionsDB;
pub use adapter_lmdb::init;

// FIXME: fabinfra/fabaccess/bffh#3
pub type UserIdentifier = u64;
pub type RoleIdentifier = u64;
pub type PermIdentifier = u64;

#[derive(Clone, Debug)]
pub struct Permissions {
    pub inner: PermissionsDB,
    env: Arc<Environment>,
}

impl Permissions {
    pub fn new(inner: PermissionsDB, env: Arc<Environment>) -> Permissions {
        Permissions { inner, env }
    }

    pub fn check(&self, userID: UserIdentifier, permID: PermIdentifier) -> Result<bool> {
        let txn = self.env.begin_ro_txn()?;
        self.inner.check(&txn, userID, permID)
    }

    pub fn get_role(&self, roleID: RoleIdentifier) -> Result<Option<Role>> {
        let txn = self.env.begin_ro_txn()?;
        self.inner.get_role(&txn, roleID)
    }
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
pub struct Role {
    name: String,

    /// A Role can have parents, inheriting all permissions
    ///
    /// This makes situations where different levels of access are required easier: Each higher
    /// level of access sets the lower levels of access as parent, inheriting their permission; if
    /// you are allowed to manage a machine you are then also allowed to use it and so on
    parents: Vec<RoleIdentifier>,
    permissions: Vec<PermIdentifier>,
}

type SourceID = String;

/// Universal (relative) id of a role
enum RoleID {
    /// The role comes from this instance
    Local {
        /// Locally unique name for the role. No other role at this instance no matter the source
        /// may have the same name
        name: String,
        /// Role Source, i.e. the database the role comes from
        source: SourceID,
    },
    /// The role comes from a federated instance
    Remote {
        /// Name of the role. This role is unique in that instance so the tuple (name, location)
        /// refers to a unique role
        name: String,
        /// The federated instance this role comes from
        location: String,
    }
}
impl fmt::Display for RoleID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            RoleID::Local {name, source} => write!(f, "{}/{}@local", name, source),
            RoleID::Remote {name, location} => write!(f, "{}@{}", name, location),
        }
    }
}
