//! Access control logic
//!

use std::cmp::Ordering;
use std::convert::{Into, TryFrom};
use std::fmt;

fn is_sep_char(c: char) -> bool {
    c == '.'
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
/// A set of privileges to a thing
pub struct PrivilegesBuf {
    /// Which permission is required to know about the existance of this thing
    pub disclose: PermissionBuf,
    /// Which permission is required to read this thing
    pub read: PermissionBuf,
    /// Which permission is required to write parts of this thing
    pub write: PermissionBuf,
    /// Which permission is required to manage all parts of this thing
    pub manage: PermissionBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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
    #[inline(always)]
    /// Allocate an empty `PermissionBuf`
    pub fn new() -> Self {
        PermissionBuf {
            inner: String::new(),
        }
    }

    #[inline(always)]
    /// Allocate a `PermissionBuf` with the given capacity given to the internal [`String`]
    pub fn with_capacity(cap: usize) -> Self {
        PermissionBuf {
            inner: String::with_capacity(cap),
        }
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
        let need_sep = self
            .inner
            .chars()
            .rev()
            .next()
            .map(|c| !is_sep_char(c))
            .unwrap_or(false);
        if need_sep {
            self.inner.push('.')
        }
        self.inner.push_str(perm.as_str())
    }

    #[inline(always)]
    pub const fn from_string_unchecked(inner: String) -> Self {
        Self { inner }
    }

    #[inline]
    pub fn from_perm(perm: &Permission) -> Self {
        Self {
            inner: perm.as_str().to_string(),
        }
    }

    #[inline(always)]
    pub fn into_string(self) -> String {
        self.inner
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}
impl AsRef<String> for PermissionBuf {
    #[inline(always)]
    fn as_ref(&self) -> &String {
        &self.inner
    }
}
impl AsRef<str> for PermissionBuf {
    #[inline(always)]
    fn as_ref(&self) -> &str {
        &self.inner.as_str()
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

#[derive(PartialEq, Eq, Hash, Debug)]
#[repr(transparent)]
/// A borrowed permission string
///
/// Permissions have total equality and partial ordering.
/// Specifically permissions on the same path in a tree can be compared for specificity.
/// This means that ```(bffh.perm) > (bffh.perm.sub) == true```
/// but ```(bffh.perm) > (unrelated.but.more.specific.perm) == false```.
/// This allows to check if PermRule a grants Perm b by checking `a > b`.
pub struct Permission(str);
impl Permission {
    #[inline(always)]
    // We can't make this `const` just yet because `str` is always a fat pointer meaning we can't
    // just const cast it, and `CoerceUnsized` and friends are currently unstable.
    pub fn new<S: AsRef<str> + ?Sized>(s: &S) -> &Permission {
        // Safe because s is a valid reference
        unsafe { &*(s.as_ref() as *const str as *const Permission) }
    }

    #[inline(always)]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[inline(always)]
    pub fn iter(&self) -> std::str::Split<char> {
        self.0.split('.')
    }
}

impl PartialOrd for Permission {
    fn partial_cmp(&self, other: &Permission) -> Option<Ordering> {
        let mut i = self.iter();
        let mut j = other.iter();
        let (mut l, mut r);
        while {
            l = i.next();
            r = j.next();

            l.is_some() && r.is_some()
        } {
            if l.unwrap() != r.unwrap() {
                return None;
            }
        }

        match (l, r) {
            (None, None) => Some(Ordering::Equal),
            (Some(_), None) => Some(Ordering::Less),
            (None, Some(_)) => Some(Ordering::Greater),
            (Some(_), Some(_)) => unreachable!(
                "Broken contract in Permission::partial_cmp: sides \
            should never be both Some!"
            ),
        }
    }
}

impl AsRef<Permission> for Permission {
    #[inline]
    fn as_ref(&self) -> &Permission {
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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
    // This lacks what LDAP calls "ONELEVEL": The ability to grant the exact children but not several
    // levels deep, i.e. `Onelevel("bffh.perm")` grants bffh.perm.sub *BUT NOT* bffh.perm.sub.two or
    // bffh.perm itself.
    // I can't think of a reason to use that so I'm skipping it for now.
}

impl PermRule {
    // Does this rule match that permission
    pub fn match_perm<P: AsRef<Permission> + ?Sized>(&self, perm: &P) -> bool {
        match self {
            PermRule::Base(ref base) => base.as_permission() == perm.as_ref(),
            PermRule::Children(ref parent) => parent.as_permission() > perm.as_ref(),
            PermRule::Subtree(ref parent) => parent.as_permission() >= perm.as_ref(),
        }
    }
}

impl fmt::Display for PermRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PermRule::Base(perm) => write!(f, "{}", perm),
            PermRule::Children(parent) => write!(f, "{}.+", parent),
            PermRule::Subtree(parent) => write!(f, "{}.*", parent),
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
            }
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
            match &input[len - 2..len] {
                ".+" => {
                    input.truncate(len - 2);
                    Ok(PermRule::Children(PermissionBuf::from_string_unchecked(
                        input,
                    )))
                }
                ".*" => {
                    input.truncate(len - 2);
                    Ok(PermRule::Subtree(PermissionBuf::from_string_unchecked(
                        input,
                    )))
                }
                _ => Ok(PermRule::Base(PermissionBuf::from_string_unchecked(input))),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_ord_test() {
        assert!(
            PermissionBuf::from_string_unchecked("bffh.perm".to_string())
                > PermissionBuf::from_string_unchecked("bffh.perm.sub".to_string())
        );
    }

    #[test]
    fn permission_simple_check_test() {
        let perm = PermissionBuf::from_string_unchecked("test.perm".to_string());
        let rule = PermRule::Base(perm.clone());

        assert!(rule.match_perm(&perm));
    }

    #[test]
    fn permission_children_checks_only_children() {
        let perm = PermissionBuf::from_string_unchecked("test.perm".to_string());
        let rule = PermRule::Children(perm.clone());

        assert_eq!(rule.match_perm(&perm), false);

        let perm2 = PermissionBuf::from_string_unchecked("test.perm.child".to_string());
        let perm3 = PermissionBuf::from_string_unchecked("test.perm.child.deeper".to_string());
        assert!(rule.match_perm(&perm2));
        assert!(rule.match_perm(&perm3));
    }

    #[test]
    fn permission_subtree_checks_base() {
        let perm = PermissionBuf::from_string_unchecked("test.perm".to_string());
        let rule = PermRule::Subtree(perm.clone());

        assert!(rule.match_perm(&perm));

        let perm2 = PermissionBuf::from_string_unchecked("test.perm.child".to_string());
        let perm3 = PermissionBuf::from_string_unchecked("test.perm.child.deeper".to_string());

        assert!(rule.match_perm(&perm2));
        assert!(rule.match_perm(&perm3));
    }

    #[test]
    fn format_and_read_compatible() {
        use std::convert::TryInto;

        let testdata = vec![("testrole", "testsource"), ("", "norole"), ("nosource", "")]
            .into_iter()
            .map(|(n, s)| (n.to_string(), s.to_string()));

        for (name, source) in testdata {
            let role = RoleIdentifier { name, source };

            let fmt_string = format!("{}", &role);

            println!("{:?} is formatted: {}", &role, &fmt_string);

            let parsed: RoleIdentifier = fmt_string.try_into().unwrap();

            println!("Which parses into {:?}", &parsed);

            assert_eq!(role, parsed);
        }
    }

    #[test]
    fn rules_from_string_test() {
        assert_eq!(
            PermRule::Base(PermissionBuf::from_string_unchecked(
                "bffh.perm".to_string()
            )),
            PermRule::try_from("bffh.perm".to_string()).unwrap()
        );
        assert_eq!(
            PermRule::Children(PermissionBuf::from_string_unchecked(
                "bffh.perm".to_string()
            )),
            PermRule::try_from("bffh.perm.+".to_string()).unwrap()
        );
        assert_eq!(
            PermRule::Subtree(PermissionBuf::from_string_unchecked(
                "bffh.perm".to_string()
            )),
            PermRule::try_from("bffh.perm.*".to_string()).unwrap()
        );
    }

    #[test]
    fn rules_from_string_edgecases_test() {
        assert!(PermRule::try_from("*".to_string()).is_err());
        assert!(PermRule::try_from("+".to_string()).is_err());
    }
}
