use crate::authorization::permissions::{PermRule, Permission};
use crate::users::db::UserData;
use once_cell::sync::OnceCell;
use std::collections::{HashMap, HashSet};
use std::fmt;

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

    /// Tally a role dependency tree into a set
    ///
    /// A Default implementation exists which adapter may overwrite with more efficient
    /// implementations.
    fn tally_role(&self, roles: &mut HashMap<String, Role>, role_id: &String) {
        if let Some(role) = self.get(role_id) {
            // Only check and tally parents of a role at the role itself if it's the first time we
            // see it
            if !roles.contains_key(role_id) {
                for parent in role.parents.iter() {
                    self.tally_role(roles, parent);
                }

                roles.insert(role_id.clone(), role.clone());
            }
        }
    }

    fn collect_permrules(&self, user: &UserData) -> Vec<PermRule> {
        let mut roleset = HashMap::new();
        for role_id in user.roles.iter() {
            self.tally_role(&mut roleset, role_id);
        }

        let mut output = Vec::new();

        // Iter all unique role->permissions we've found and early return on match.
        for (_roleid, role) in roleset.iter() {
            output.extend(role.permissions.iter().cloned())
        }

        output
    }

    fn permitted_tally(
        &self,
        roles: &mut HashSet<String>,
        role_id: &String,
        perm: &Permission,
    ) -> bool {
        let _guard = tracing::debug_span!("tally", %role_id, perm=perm.as_str());
        if let Some(role) = self.get(role_id) {
            // Only check and tally parents of a role at the role itself if it's the first time we
            // see it
            if !roles.contains(role_id) {
                for perm_rule in role.permissions.iter() {
                    if perm_rule.match_perm(perm) {
                        tracing::debug!("Permission granted by direct role");
                        return true;
                    }
                }
                for parent in role.parents.iter() {
                    if self.permitted_tally(roles, parent, perm) {
                        tracing::debug!(%parent, "Permission granted by parent role");
                        return true;
                    }
                }

                roles.insert(role_id.clone());
            }
        }

        tracing::trace!(%role_id, "Permission not granted by role");
        false
    }

    pub fn is_permitted(&self, user: &UserData, perm: impl AsRef<Permission>) -> bool {
        let perm = perm.as_ref();
        tracing::debug!(perm = perm.as_str(), "Checking permission");
        let mut seen = HashSet::new();
        for role_id in user.roles.iter() {
            if self.permitted_tally(&mut seen, role_id, perm.as_ref()) {
                return true;
            }
        }
        false
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
        Self {
            parents,
            permissions,
        }
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
