use serde::{Serialize, Deserialize};
use std::fmt;
use crate::db::access::RoleIdentifier;
use std::collections::HashMap;

/// A Person, from the Authorization perspective
#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct AuthzContext {
    /// The identification of this user.
    pub id: UserIdentifier,

    /// A Person has N ≥ 0 roles.
    /// Persons are only ever given roles, not permissions directly
    pub roles: Vec<RoleIdentifier>,

    /// Additional data storage
    #[serde(flatten)]
    kv: HashMap<Box<[u8]>, Box<[u8]>>,
}

impl fmt::Display for UserIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = write!(f, "{}", self.uid)?;
        if let Some(ref s) = self.subuid {
            write!(f, "+{}", s)?;
        }
        if let Some(ref l) = self.location {
            write!(f, "@{}", l)?;
        }
        Ok(r)
    }
}

/// User Database Trait
pub trait UserDB {
    fn get_user(&self, uid: UserIdentifier) -> Option<User>;
}
