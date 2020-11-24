//! UserDB does two kinds of lookups:
//! 1. "I have this here username, what user is that"
//! 2. "I have this here user, what are their roles (and other associated data)"
use serde::{Serialize, Deserialize};
use std::fmt;
use crate::db::access::RoleIdentifier;
use std::collections::HashMap;

mod internal;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub id: UserId,
    pub data: UserData,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
/// Authorization Identity
///
/// This identity is internal to FabAccess and completely independent from the authentication
/// method or source
pub struct UserId {
    /// Main User ID. Generally an user name or similar
    uid: String,
    /// Sub user ID. 
    ///
    /// Can change scopes for permissions, e.g. having a +admin account with more permissions than
    /// the default account and +dashboard et.al. accounts that have restricted permissions for
    /// their applications
    subuid: Option<String>,
    /// Realm this account originates.
    ///
    /// The Realm is usually described by a domain name but local policy may dictate an unrelated
    /// mapping
    realm: Option<String>,
}

impl UserId {
    pub fn new(uid: String, subuid: Option<String>, realm: Option<String>) -> Self {
        Self { uid, subuid, realm }
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = write!(f, "{}", self.uid);
        if let Some(ref s) = self.subuid {
            write!(f, "+{}", s)?;
        }
        if let Some(ref l) = self.realm {
            write!(f, "@{}", l)?;
        }
        r
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
/// A Person, from the Authorization perspective
pub struct UserData {
    /// A Person has N ≥ 0 roles.
    /// Persons are only ever given roles, not permissions directly
    pub roles: Vec<RoleIdentifier>,

    /// Additional data storage
    #[serde(flatten)]
    kv: HashMap<Box<[u8]>, Box<[u8]>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_uid_test() {
        let uid = "testuser".to_string();
        let suid = "testsuid".to_string();
        let realm = "testloc".to_string();

        assert_eq!("testuser", 
            format!("{}", UserIdentifier::new(uid.clone(), None, None)));
        assert_eq!("testuser+testsuid", 
            format!("{}", UserIdentifier::new(uid.clone(), Some(suid.clone()), None)));
        assert_eq!("testuser+testsuid", 
            format!("{}", UserIdentifier::new(uid.clone(), Some(suid.clone()), None)));
        assert_eq!("testuser+testsuid@testloc", 
            format!("{}", UserIdentifier::new(uid, Some(suid), Some(realm))));
    }
}
