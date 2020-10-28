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

pub trait RoleDB {
    fn get_role(&self, roleID: RoleIdentifier) -> Result<Option<Role>>;

    /// Check if a given user has the given permission
    /// 
    /// Default implementation which adapter may overwrite with more efficient specialized
    /// implementations.
    fn check(&self, user: &User, permID: PermIdentifier) -> Result<bool> {
        self.check_roles(user.roles)
    }

    /// Check if a given permission is granted by any of the given roles or their respective
    /// parents
    /// 
    /// Default implementation which adapter may overwrite with more efficient specialized
    /// implementations.
    fn check_roles(&self, roles: &[RoleIdentifier], permID: PermIdentifier) -> Result<bool> {
        // Tally all roles. Makes dependent roles easier
        let mut roles = HashSet::new();
        for roleID in roles {
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

        return Ok(false);
    }

    /// Tally a role dependency tree into a set
    ///
    /// Default implementation which adapter may overwrite with more efficient implementations
    fn tally_role(&self, roles: &mut HashSet<Role>, roleID: RoleIdentifier) -> Result<()> {
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
enum RoleIdentifier {
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
            RoleIdentifier::Local {name, source} => write!(f, "{}/{}@local", name, source),
            RoleIdentifier::Remote {name, location} => write!(f, "{}@{}", name, location),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// An identifier for a permission
// XXX: Does remote permissions ever make sense?
// I mean we kinda get them for free so maybe?
pub enum PermIdentifier {
    Local(PermRule),
    Remote(PermRule, String),
}
impl fmt::Display for PermIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            PermIdentifier::Local(perm) 
                => write!(f, "{}", perm),
            PermIdentifier::Remote(perm, source) 
                => write!(f, "{}@{}", perm, source),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[repr(transparent)]
/// An owned permission string
///
/// This is under the hood just a fancy std::String.
// TODO: What is the possible fallout from homograph attacks?
// i.e. "bffh.perm" is not the same as "bffհ.реrm" (Armenian 'հ':Հ and Cyrillic 'е':Е)
// See also https://util.unicode.org/UnicodeJsps/confusables.jsp
pub struct PermissionBuf {
    inner: String,
}
impl PermissionBuf {
    /// Allocate an empty `PermissionBuf`
    pub fn new() -> Self {
        PermissionBuf { inner: String::new() }
    }

    /// Allocate a `PermissionBuf` with the given capacity given to the internal [`String`]
    pub fn with_capacity() -> Self {
        PermissionBuf { inner: String::with_capacity() }
    }

    #[inline(always)]
    pub fn as_permission(&self) -> &Permission {
        self
    }

    pub fn push<P: AsRef<Permission>>(&mut self, perm: P) {
        self._push(perm.as_ref())
    }

    pub fn _push(&mut self, perm: &Permission) {
        // in general we always need a separator unless the last byte is one or the string is empty
        let need_sep = self.inner.chars().rev().next().map(|c| !is_sep_char(c)).unwrap_or(false);
        if need_sep {
            self.inner.push('.')
        }
        self.inner.push(perm.as_str())
    }

    pub fn from_string(inner: String) -> Self {
        Self { inner }
    }
}
impl AsRef<Permission> for PermissionBuf {
    #[inline(always)]
    fn as_ref(&self) -> &Permission {
        self.as_permission()
    }
}

#[repr(transparent)]
#[derive(PartialEq, Eq)]
/// A borrowed permission string
/// 
/// Permissions have total equality and partial ordering.
/// Specifically permissions on the same path in a tree can be compared for specificity.
/// This means that ```(bffh.perm) > (bffh.perm.sub) == true```
/// but ```(bffh.perm) > (unrelated.but.specific.perm) == false```
pub struct Permission {
    inner: str
}
impl Permission {
    pub fn as_str(&self) -> &str {
        self.inner
    }

    pub fn iter(&self) -> std::str::Split<Char>  {
        self.inner.split('.')
    }
}

impl PartialOrd for Permission {
    fn partial_cmp(&self, other: &Permission) -> Option<Ordering> {
        let (l,r) = (None, None);
        while {
            l = self.next();
            r = other.next();

            l.is_some() && r.is_some()
        } {
            if l.unwrap() != r.unwrap() {
                return None;
            }
        }

        match (l,r) {
            (None, None) => Some(Ordering::Equal),
            (Some(_), None) => Some(Ordering::Lesser),
            (None, Some(_)) => Some(Ordering::Greater),
            (Some(_), Some(_)) => panic!("Broken contract in Permission::partial_cmp: sides should never be both Some!"),
        }
    }
}


#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermRule {
    /// The permission is precise, 
    ///
    /// i.e. `Base("bffh.perm")` grants bffh.perm but does not grant permission for bffh.perm.sub
    Base(PermissionBuf),
    /// The permissions is for the children of the node
    ///
    /// i.e. `Children("bffh.perm")` grants bffh.perm.sub, bffh.perm.sub.two *BUT NOT* bffh.perm
    /// itself.
    Children(PermissionBuf),
    /// The permissions is for the subtree marked by the node
    ///
    /// i.e. `Children("bffh.perm")` grants bffh.perm.sub, bffh.perm.sub.two and also bffh.perm
    /// itself.
    Subtree(PermissionBuf),
    // This lacks what LDAP calls ONELEVEL: The ability to grant the exact children but not several
    // levels deep, i.e. Onelevel("bffh.perm") grants bffh.perm.sub *BUT NOT* bffh.perm.sub.two or
    // bffh.perm itself.
    // I can't think of a reason to use that so I'm skipping it for now.
}

impl PermRule {
    // Does this rule match that permission
    fn match_perm<P: AsRef<Permission>>(rule: &PermRule, perm: P) -> bool {
        match rule {
            Base(base) => base == perm,
            Children(parent) => parent > perm ,
            Subtree(parent) => parent >= perm,
        }
    }
}

impl fmt::Display for PermRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            PermRule::Base(perm)
                => write!(f, "{}", perm),
            PermRule::Children(parent)
                => write!(f,"{}.+", parent),
            PermRule::Subtree(parent)
                => write!(f,"{}.*", parent),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_ord_test() {
        assert!(PermissionBuf::from_string("bffh.perm") > PermissionBuf::from_string("bffh.perm.sub"));
    }
}
