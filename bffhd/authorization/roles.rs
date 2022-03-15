use std::collections::HashMap;
use std::fmt;
use once_cell::sync::OnceCell;
use crate::authorization::permissions::PermRule;

static ROLES: OnceCell<HashMap<String, Role>> = OnceCell::new();

#[derive(Copy, Clone)]
pub struct Roles {
    roles: &'static HashMap<String, Role>,
}

impl Roles {
    pub fn new(roles: HashMap<String, Role>) -> Self {
        let span = tracing::debug_span!("roles", "Creating Roles handle");
        let _guard = span.enter();

        let this = ROLES.get_or_init(|| {
            tracing::debug!("Initializing global roles…");
            roles
        });
        Self { roles: this }
    }

    pub fn get(self, roleid: &str) -> Option<&Role> {
        self.roles.get(roleid)
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
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Role {
    // If a role doesn't define parents, default to an empty Vec.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// A Role can have parents, inheriting all permissions
    ///
    /// This makes situations where different levels of access are required easier: Each higher
    /// level of access sets the lower levels of access as parent, inheriting their permission; if
    /// you are allowed to manage a machine you are then also allowed to use it and so on
    parents: Vec<String>,

    // If a role doesn't define permissions, default to an empty Vec.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    permissions: Vec<PermRule>,
}

impl Role {
    pub fn new(parents: Vec<String>, permissions: Vec<PermRule>) -> Self {
        Self { parents, permissions }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "parents:")?;
        if self.parents.is_empty() {
            writeln!(f, " []")?;
        } else {
            writeln!(f, "")?;
            for p in self.parents.iter() {
                writeln!(f, "  - {}", p)?;
            }
        }
        write!(f, "permissions:")?;
        if self.permissions.is_empty() {
            writeln!(f, " []")?;
        } else {
            writeln!(f, "")?;
            for p in self.permissions.iter() {
                writeln!(f, "  - {}", p)?;
            }
        }

        Ok(())
    }
}