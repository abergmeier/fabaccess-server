use std::fmt;
use crate::authorization::permissions::PermRule;

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
    parents: Vec<RoleIdentifier>,

    // If a role doesn't define permissions, default to an empty Vec.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    permissions: Vec<PermRule>,
}

impl Role {
    pub fn new(parents: Vec<RoleIdentifier>, permissions: Vec<PermRule>) -> Self {
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

type SourceID = String;

fn split_once(s: &str, split: char) -> Option<(&str, &str)> {
    s
        .find(split)
        .map(|idx| (&s[..idx], &s[(idx+1)..]))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String")]
#[serde(into = "String")]
/// Universal (relative) id of a role
pub struct RoleIdentifier {
    /// Locally unique name for the role. No other role at this instance no matter the source
    /// may have the same name
    name: String,
    /// Role Source, i.e. the database the role comes from
    source: SourceID,
}

impl RoleIdentifier {
    pub fn new<>(name: &str, source: &str) -> Self {
        Self { name: name.to_string(), source: source.to_string() }
    }
    pub fn from_strings(name: String, source: String) -> Self {
        Self { name, source }
    }
}

impl fmt::Display for RoleIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.source != "" {
            write!(f, "{}/{}", self.name, self.source)
        } else {
            write!(f, "{}", self.name)
        }
    }
}

impl std::str::FromStr for RoleIdentifier {
    type Err = RoleFromStrError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if let Some((name, source)) = split_once(s, '/') {
            Ok(RoleIdentifier { name: name.to_string(), source: source.to_string() })
        } else {
            Ok(RoleIdentifier { name: s.to_string(), source: String::new() })
        }
    }
}

impl TryFrom<String> for RoleIdentifier {
    type Error = RoleFromStrError;

    fn try_from(s: String) -> std::result::Result<Self, Self::Error> {
        <RoleIdentifier as std::str::FromStr>::from_str(&s)
    }
}
impl Into<String> for RoleIdentifier {
    fn into(self) -> String {
        format!("{}", self)
    }
}

impl RoleIdentifier {
    pub fn local_from_str(source: String, name: String) -> Self {
        RoleIdentifier { name, source }
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
