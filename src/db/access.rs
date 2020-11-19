//! Access control logic
//!

use std::fmt;
use std::collections::HashSet;
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::fs;
use std::io::Write;
use std::sync::Arc;
use std::collections::HashMap;
use std::iter::FromIterator;
use std::convert::{TryFrom, Into};

use flexbuffers;
use serde::{
    Serialize,
    Serializer,

    Deserialize,
    Deserializer,
};

use slog::Logger;
use lmdb::{Environment, Transaction, RwTransaction, Cursor};

use crate::config::Settings;
use crate::error::Result;

pub mod internal;

use crate::db::user::User;
pub use internal::init;

pub trait RoleDB {
    fn get_role(&self, roleID: &RoleIdentifier) -> Result<Option<Role>>;

    /// Check if a given user has the given permission
    /// 
    /// Default implementation which adapter may overwrite with more efficient specialized
    /// implementations.
    fn check<P: AsRef<Permission>>(&self, user: &User, perm: &P) -> Result<bool> {
        self.check_roles(&user.roles, perm)
    }

    /// Check if a given permission is granted by any of the given roles or their respective
    /// parents
    ///
    /// A Default implementation exists which adapter may overwrite with more efficient specialized
    /// implementations.
    fn check_roles<P: AsRef<Permission>>(&self, roles: &[RoleIdentifier], perm: &P) -> Result<bool> {
        // Tally all roles. Makes dependent roles easier
        let mut roleset = HashSet::new();
        for roleID in roles {
            self.tally_role(&mut roleset, roleID)?;
        }

        // Iter all unique role->permissions we've found and early return on match. 
        for role in roleset.iter() {
            for perm_rule in role.permissions.iter() {
                if perm_rule.match_perm(perm) {
                    return Ok(true);
                }
            }
        }

        return Ok(false);
    }

    /// Tally a role dependency tree into a set
    ///
    /// A Default implementation exists which adapter may overwrite with more efficient
    /// implementations.
    fn tally_role(&self, roles: &mut HashSet<Role>, roleID: &RoleIdentifier) -> Result<()> {
        if let Some(role) = self.get_role(roleID)? {
            // Only check and tally parents of a role at the role itself if it's the first time we
            // see it
            if !roles.contains(&role) {
                for parent in role.parents.iter() {
                    self.tally_role(roles, parent)?;
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

    // If a role doesn't define parents, default to an empty Vec.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// A Role can have parents, inheriting all permissions
    ///
    /// This makes situations where different levels of access are required easier: Each higher
    /// level of access sets the lower levels of access as parent, inheriting their permission; if
    /// you are allowed to manage a machine you are then also allowed to use it and so on
    parents: Vec<RoleIdentifier>,

    // If a role doesn't define permissions, default to an empty Vec.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    permissions: Vec<PermRule>,
}

impl Role {
    fn load_file<P: AsRef<Path>>(path: P) -> Result<HashMap<RoleIdentifier, Role>> {
        let content = fs::read(path)?;
        let file_roles: HashMap<String, Role> = toml::from_slice(&content[..])?;

        Ok(HashMap::from_iter(file_roles.into_iter().map(|(key, value)| {
            (RoleIdentifier::local_from_str("lmdb".to_string(), key), value)
        })))
    }
}

type SourceID = String;

fn split_once(s: &str, split: char) -> Option<(&str, &str)> {
    s
        .find(split)
        .map(|idx| s.split_at(idx))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String")]
/// Universal (relative) id of a role
pub enum RoleIdentifier {
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

impl fmt::Display for RoleIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RoleIdentifier::Local {name, source} => write!(f, "{}/{}@local", name, source),
            RoleIdentifier::Remote {name, location} => write!(f, "{}@{}", name, location),
        }
    }
}

impl std::str::FromStr for RoleIdentifier {
    type Err = RoleFromStrError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if let Some((name, location)) = split_once(s, '@') {
            Ok(RoleIdentifier::Remote { name: name.to_string(), location: location.to_string() })
        } else if let Some((name, source)) = split_once(s, '%') {
            Ok(RoleIdentifier::Local { name: name.to_string(), source: source.to_string() })
        } else {
            Err(RoleFromStrError::Invalid)
        }
    }
}

impl TryFrom<String> for RoleIdentifier {
    type Error = RoleFromStrError;

    fn try_from(s: String) -> std::result::Result<Self, Self::Error> {
        if let Some((name, location)) = split_once(&s, '@') {
            let location = &location[1..];
            Ok(RoleIdentifier::Remote { name: name.to_string(), location: location.to_string() })
        } else if let Some((name, source)) = split_once(&s, '%') {
            let source = &source[1..];
            Ok(RoleIdentifier::Local { name: name.to_string(), source: source.to_string() })
        } else {
            Err(RoleFromStrError::Invalid)
        }
    }
}

impl RoleIdentifier {
    pub fn local_from_str(source: String, name: String) -> Self {
        RoleIdentifier::Local { name, source }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RoleFromStrError {
    /// No '@' or '%' found. That's strange, huh?
    Invalid
}

impl fmt::Display for RoleFromStrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RoleFromStrError::Invalid 
                => write!(f, "Rolename are of form 'name%source' or 'name@realm'."),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
/// An identifier for a permission
// XXX: Does remote permissions ever make sense?
// I mean we kinda get them for free so maybe?
pub enum PermIdentifier {
    Local(PermRule),
    Remote(PermRule, String),
}
impl fmt::Display for PermIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PermIdentifier::Local(perm) 
                => write!(f, "{}", perm),
            PermIdentifier::Remote(perm, source) 
                => write!(f, "{}@{}", perm, source),
        }
    }
}

fn is_sep_char(c: char) -> bool {
    c == '.'
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
/// A set of privileges to a thing
pub struct PrivilegesBuf {
    /// Which permission is required to know about the existance of this thing
    pub disclose: PermissionBuf,
    /// Which permission is required to read this thing
    pub read: PermissionBuf,
    /// Which permission is required to write parts of this thing
    pub write: PermissionBuf,
    /// Which permission is required to manage all parts of this thing
    pub manage: PermissionBuf
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
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
    pub fn with_capacity(cap: usize) -> Self {
        PermissionBuf { inner: String::with_capacity(cap) }
    }

    #[inline(always)]
    pub fn as_permission(&self) -> &Permission {
        self.as_ref()
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
        self.inner.push_str(perm.as_str())
    }

    pub fn from_string(inner: String) -> Self {
        Self { inner }
    }

    pub fn into_string(self) -> String {
        self.inner
    }
}
impl AsRef<str> for PermissionBuf {
    #[inline(always)]
    fn as_ref(&self) -> &str {
        &self.inner[..]
    }
}
impl AsRef<Permission> for PermissionBuf {
    #[inline]
    fn as_ref(&self) -> &Permission {
        Permission::new(self)
    }
}
impl PartialOrd for PermissionBuf {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let a: &Permission = self.as_ref();
        a.partial_cmp(other.as_ref())
    }
}
impl fmt::Display for PermissionBuf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

#[repr(transparent)]
#[derive(PartialEq, Eq, Hash)]
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
    pub fn new<S: AsRef<str> + ?Sized>(s: &S) -> &Permission {
        unsafe { &*(s.as_ref() as *const str as *const Permission) }
    }

    pub fn as_str(&self) -> &str {
        &self.inner
    }

    pub fn iter(&self) -> std::str::Split<char>  {
        self.inner.split('.')
    }
}

impl PartialOrd for Permission {
    fn partial_cmp(&self, other: &Permission) -> Option<Ordering> {
        let mut i = self.iter();
        let mut j = other.iter();
        let (mut l, mut r) = (None, None);
        while {
            l = i.next();
            r = j.next();

            l.is_some() && r.is_some()
        } {
            if l.unwrap() != r.unwrap() {
                return None;
            }
        }

        match (l,r) {
            (None, None) => Some(Ordering::Equal),
            (Some(_), None) => Some(Ordering::Less),
            (None, Some(_)) => Some(Ordering::Greater),
            (Some(_), Some(_)) => panic!("Broken contract in Permission::partial_cmp: sides should never be both Some!"),
        }
    }
}

impl AsRef<Permission> for Permission {
    #[inline]
    fn as_ref(&self) -> &Permission {
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String")]
#[serde(into = "String")]
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
    fn match_perm<P: AsRef<Permission>>(&self, perm: &P) -> bool {
        match self {
            PermRule::Base(ref base) => base.as_permission() == perm.as_ref(),
            PermRule::Children(ref parent) => parent.as_permission() > perm.as_ref() ,
            PermRule::Subtree(ref parent) => parent.as_permission() >= perm.as_ref(),
        }
    }
}

impl fmt::Display for PermRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PermRule::Base(perm)
                => write!(f, "{}", perm),
            PermRule::Children(parent)
                => write!(f,"{}.+", parent),
            PermRule::Subtree(parent)
                => write!(f,"{}.*", parent),
        }
    }
}

impl Into<String> for PermRule {
    fn into(self) -> String {
        match self {
            PermRule::Base(perm) => perm.into_string(),
            PermRule::Children(mut perm) => {
                perm.push(Permission::new("+"));
                perm.into_string()
            },
            PermRule::Subtree(mut perm) => {
                perm.push(Permission::new("+"));
                perm.into_string()
            }
        }
    }
}

impl TryFrom<String> for PermRule {
    type Error = &'static str;

    fn try_from(mut input: String) -> std::result::Result<Self, Self::Error> {
        // Check out specifically the last two chars
        let len = input.len();
        if len <= 2 {
            Err("Input string for PermRule is too short")
        } else {
            match &input[len-2..len] {
                ".+" => {
                    input.truncate(len-2);
                    Ok(PermRule::Children(PermissionBuf::from_string(input)))
                },
                ".*" => {
                    input.truncate(len-2);
                    Ok(PermRule::Subtree(PermissionBuf::from_string(input)))
                },
                _ => Ok(PermRule::Base(PermissionBuf::from_string(input))),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_ord_test() {
        assert!(PermissionBuf::from_string("bffh.perm".to_string()) 
            > PermissionBuf::from_string("bffh.perm.sub".to_string()));
    }

    #[test]
    fn permission_simple_check_test() {
        let perm = PermissionBuf::from_string("test.perm".to_string());
        let rule = PermRule::Base(perm.clone());

        assert!(rule.match_perm(&perm));
    }

    #[test]
    #[should_panic]
    fn permission_children_checks_only_children() {
        let perm = PermissionBuf::from_string("test.perm".to_string());
        let rule = PermRule::Children(perm.clone());

        assert!(rule.match_perm(&perm));
    }

    #[test]
    fn load_examples_roles_test() {
        let mut roles = Role::load_file("examples/roles.toml")
            .expect("Couldn't load the example role defs. Does `examples/roles.toml` exist?");

        let expected = vec![
            (RoleIdentifier::Local { name: "testrole".to_string(), source: "lmdb".to_string() },
            Role {
                name: "Testrole".to_string(),
                parents: vec![],
                permissions: vec![
                    PermRule::Subtree(PermissionBuf::from_string("lab.test".to_string()))
                ],
            }),
            (RoleIdentifier::Local { name: "somerole".to_string(), source: "lmdb".to_string() },
            Role {
                name: "Somerole".to_string(),
                parents: vec![
                    RoleIdentifier::local_from_str("lmdb".to_string(), "testparent".to_string()),
                ],
                permissions: vec![
                    PermRule::Base(PermissionBuf::from_string("lab.some.admin".to_string()))
                ],
            }),
            (RoleIdentifier::Local { name: "testparent".to_string(), source: "lmdb".to_string() },
            Role {
                name: "Testparent".to_string(),
                parents: vec![],
                permissions: vec![
                    PermRule::Base(PermissionBuf::from_string("lab.some.write".to_string())),
                    PermRule::Base(PermissionBuf::from_string("lab.some.read".to_string())),
                    PermRule::Base(PermissionBuf::from_string("lab.some.disclose".to_string())),
                ],
            }),
        ];

        for (id, role) in expected {
            assert_eq!(roles.remove(&id).unwrap(), role);
        }

        assert!(roles.is_empty())
    }

    #[test]
    fn rules_from_string_test() {
        assert_eq!(
            PermRule::Base(PermissionBuf::from_string("bffh.perm".to_string())),
            PermRule::try_from("bffh.perm".to_string()).unwrap()
        );
        assert_eq!(
            PermRule::Children(PermissionBuf::from_string("bffh.perm".to_string())),
            PermRule::try_from("bffh.perm.+".to_string()).unwrap()
        );
        assert_eq!(
            PermRule::Subtree(PermissionBuf::from_string("bffh.perm".to_string())),
            PermRule::try_from("bffh.perm.*".to_string()).unwrap()
        );
    }

    #[test]
    fn rules_from_string_edgecases_test() {
        assert!(PermRule::try_from("*".to_string()).is_err());
        assert!(PermRule::try_from("+".to_string()).is_err());
    }
}
