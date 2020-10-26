use serde::{Serialize, Deserialize};
use std::fmt;
use crate::db::access::RoleIdentifier;

/// A Person, from the Authorization perspective
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct User {
    name: String,

    /// A Person has N ≥ 0 roles.
    /// Persons are only ever given roles, not permissions directly
    pub roles: Vec<RoleIdentifier>
}


/// Locally unique identifier for an user
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserIdentifier {
    /// Main UID. Must be unique in this instance so that the tuple (uid, location) is globally
    /// unique.
    uid: String,
    /// Subordinate ID. Must be unique for this user, i.e. the tuple (uid, subuid) must be unique
    /// but two different uids can have the same subuid. `None` means no subuid is set and the ID
    /// refers to the main users
    subuid: Option<String>,
    /// Location of the instance the user comes from. `None` means the local instance.
    location: Option<String>,
}

impl UserIdentifier {
    pub fn new(uid: String, subuid: Option<String>, location: Option<String>) -> Self {
        Self { uid, subuid, location }
    }
}

impl fmt::Display for UserIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = write!(f, "{}", self.uid);
        if let Some(s) = self.subuid {
            write!(f, "+{}", s)?;
        }
        if let Some(l) = self.location {
            write!(f, "@{}", l)?;
        }
        r
    }
}

/// User Database Trait
pub trait UserDB {
    fn get_user(&self, uid: UserIdentifier) -> Option<User>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_uid_test() {
        
    }
}
