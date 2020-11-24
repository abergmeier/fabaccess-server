use serde::{Serialize, Deserialize};
use std::fmt;
use crate::db::access::RoleIdentifier;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
/// Authorization Identity
///
/// This identity is internal to FabAccess and completely independent from the authentication
/// method or source
struct AuthZId {
    /// Main User ID. Generally an user name or similar
    uid: String,
    /// Sub user ID. 
    ///
    /// Can change scopes for permissions, e.g. having a +admin account with more permissions than
    /// the default account and +dashboard et.al. accounts that have restricted permissions for
    /// their applications
    subuid: String,
    /// Realm this account originates.
    ///
    /// The Realm is usually described by a domain name but local policy may dictate an unrelated
    /// mapping
    realm: String,
}

/// A Person, from the Authorization perspective
#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct AuthzContext {
    /// The identification of this user.
    pub id: AuthZId,

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
        let location = "testloc".to_string();

        assert_eq!("testuser", 
            format!("{}", UserIdentifier::new(uid.clone(), None, None)));
        assert_eq!("testuser+testsuid", 
            format!("{}", UserIdentifier::new(uid.clone(), Some(suid.clone()), None)));
        assert_eq!("testuser+testsuid", 
            format!("{}", UserIdentifier::new(uid.clone(), Some(suid.clone()), None)));
        assert_eq!("testuser+testsuid@testloc", 
            format!("{}", UserIdentifier::new(uid, Some(suid), Some(location))));
    }
}
