//! Access control logic
//!

use std::collections::HashSet;

use std::convert::TryInto;

use std::path::{Path, PathBuf};
use std::fs;
use std::io::Write;

use flexbuffers;
use serde::{Serialize, Deserialize};

use slog::Logger;
use lmdb::{Transaction, RwTransaction, Cursor};

use crate::config::Config;
use crate::error::Result;

type UserIdentifier = u64;
type RoleIdentifier = u64;
type PermIdentifier = u64;

pub struct PermissionsProvider {
    log: Logger,
    roledb: lmdb::Database,
    permdb: lmdb::Database,
    userdb: lmdb::Database,
}

impl PermissionsProvider {
    pub fn new(log: Logger, roledb: lmdb::Database, permdb: lmdb::Database, userdb: lmdb::Database) -> Self {
        Self { log, roledb, permdb, userdb }
    }

    /// Check if a given user has the given permission
    #[allow(unused)]
    pub fn check<T: Transaction>(&self, txn: &T, userID: UserIdentifier, permID: PermIdentifier) -> Result<bool> {
        if let Some(user) = self.get_user(txn, userID)? {
            // Tally all roles. Makes dependent roles easier
            let mut roles = HashSet::new();
            for roleID in user.roles {
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
        }

        return Ok(false);
    }

    fn tally_role<T: Transaction>(&self, txn: &T, roles: &mut HashSet<Role>, roleID: RoleIdentifier) -> Result<()> {
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

    fn get_role<'txn, T: Transaction>(&self, txn: &'txn T, roleID: RoleIdentifier) -> Result<Option<Role>> {
        match txn.get(self.roledb, &roleID.to_ne_bytes()) {
            Ok(bytes) => {
                Ok(Some(flexbuffers::from_slice(bytes)?))
            },
            Err(lmdb::Error::NotFound) => { Ok(None) },
            Err(e) => { Err(e.into()) }
        }
    }

    fn get_user<T: Transaction>(&self, txn: &T, userID: UserIdentifier) -> Result<Option<User>> {
        match txn.get(self.userdb, &userID.to_ne_bytes()) {
            Ok(bytes) => {
                Ok(Some(flexbuffers::from_slice(bytes)?))
            },
            Err(lmdb::Error::NotFound) => { Ok(None) },
            Err(e) => { Err(e.into()) }
        }
    }

    fn get_perm<T: Transaction>(&self, txn: &T, permID: PermIdentifier) -> Result<Option<Perm>> {
        match txn.get(self.permdb, &permID.to_ne_bytes()) {
            Ok(bytes) => {
                Ok(Some(flexbuffers::from_slice(bytes)?))
            },
            Err(lmdb::Error::NotFound) => { Ok(None) },
            Err(e) => { Err(e.into()) }
        }
    }

   fn put_role(&self, txn: &mut RwTransaction, roleID: RoleIdentifier, role: Role) -> Result<()> {
       let bytes = flexbuffers::to_vec(role)?;
       txn.put(self.roledb, &roleID.to_ne_bytes(), &bytes, lmdb::WriteFlags::empty())?;

       Ok(())
   }

   fn put_user(&self, txn: &mut RwTransaction, userID: UserIdentifier, user: User) -> Result<()> {
       let bytes = flexbuffers::to_vec(user)?;
       txn.put(self.userdb, &userID.to_ne_bytes(), &bytes, lmdb::WriteFlags::empty())?;

       Ok(())
   }

   fn put_perm(&self, txn: &mut RwTransaction, permID: PermIdentifier, perm: Perm) -> Result<()> {
       let bytes = flexbuffers::to_vec(perm)?;
       txn.put(self.permdb, &permID.to_ne_bytes(), &bytes, lmdb::WriteFlags::empty())?;

       Ok(())
   }

   pub fn dump_db<T: Transaction>(&mut self, txn: &T, mut path: PathBuf) -> Result<()> {
       path.push("roles");
       if let Err(e) = fs::create_dir(&path) {
          error!(self.log, "Failed to create 'roles' directory: {}, skipping!", e);
          return Ok(())
       } else {
           // Rust's stdlib considers the last element the file name so we have to put a dummy here for
           // .set_filename() to work correctly
           path.push("dummy");
           self.dump_roles(txn, path.clone())?;
           path.pop();
       }
       path.pop();

       path.push("perms");
       if let Err(e) = fs::create_dir(&path) {
          error!(self.log, "Failed to create 'perms' directory: {}, skipping!", e);
          return Ok(())
       } else {
           // Rust's stdlib considers the last element the file name so we have to put a dummy here for
           // .set_filename() to work correctly
           path.push("dummy");
           self.dump_perms(txn, path.clone())?;
           path.pop();
       }
       path.pop();

       path.push("users");
       if let Err(e) = fs::create_dir(&path) {
          error!(self.log, "Failed to create 'users' directory: {}, skipping!", e);
          return Ok(())
       } else {
           // Rust's stdlib considers the last element the file name so we have to put a dummy here for
           // .set_filename() to work correctly
           path.push("dummy");
           self.dump_users(txn, path.clone())?;
           path.pop();
       }
       path.pop();

       Ok(())
   }

   fn dump_roles<T: Transaction>(&mut self, txn: &T, mut path: PathBuf) -> Result<()> {
       let mut role_cursor = txn.open_ro_cursor(self.roledb)?;
       for buf in role_cursor.iter_start() {
           let (kbuf, vbuf) = buf?;
           let (kbytes, _rest) = kbuf.split_at(std::mem::size_of::<u64>());
           let roleID = u64::from_ne_bytes(kbytes.try_into().unwrap());
           let role: Role = flexbuffers::from_slice(vbuf)?;
           let filename = format!("{:x}.toml", roleID);
           path.set_file_name(filename);
           let mut fp = std::fs::File::create(&path)?;
           let toml = toml::to_vec(&role)?;
           fp.write_all(&toml)?;
       }

       Ok(())
   }

   fn dump_perms<T: Transaction>(&mut self, txn: &T, mut path: PathBuf) -> Result<()> {
       let mut perm_cursor = txn.open_ro_cursor(self.permdb)?;
       for buf in perm_cursor.iter_start() {
           let (kbuf, vbuf) = buf?;
           let (kbytes, _rest) = kbuf.split_at(std::mem::size_of::<u64>());
           let permID = u64::from_ne_bytes(kbytes.try_into().unwrap());
           let perm: Perm = flexbuffers::from_slice(vbuf)?;
           let filename = format!("{:x}.toml", permID);
           path.set_file_name(filename);
           let mut fp = std::fs::File::create(&path)?;
           let toml = toml::to_vec(&perm)?;
           fp.write_all(&toml)?;
       }

       Ok(())
   }

   fn dump_users<T: Transaction>(&mut self, txn: &T, mut path: PathBuf) -> Result<()> {
       let mut user_cursor = txn.open_ro_cursor(self.userdb)?;
       for buf in user_cursor.iter_start() {
           let (kbuf, vbuf) = buf?;
           let (kbytes, _rest) = kbuf.split_at(std::mem::size_of::<u64>());
           let userID = u64::from_ne_bytes(kbytes.try_into().unwrap());
           let user: User = flexbuffers::from_slice(vbuf)?;
           let filename = format!("{:x}.toml", userID);
           path.set_file_name(filename);
           let mut fp = std::fs::File::create(&path)?;
           let toml = toml::to_vec(&user)?;
           fp.write_all(&toml)?;
       }

       Ok(())
   }

   pub fn load_db(&mut self, txn: &mut RwTransaction, mut path: PathBuf) -> Result<()> {
       // ====================: ROLES :====================
       path.push("roles");
       if !path.is_dir() {
           error!(self.log, "Given load directory is malformed, no 'roles' subdir, not loading roles!");
       } else {
           self.load_roles(txn, path.as_path())?;
       }
       path.pop();
       // =================================================

       // ====================: PERMS :====================
       path.push("perms");
       if !path.is_dir() {
           error!(self.log, "Given load directory is malformed, no 'perms' subdir, not loading perms!");
       } else {
           //self.load_perms(txn, &path)?;
       }
       path.pop();
       // =================================================

       // ====================: USERS :====================
       path.push("users");
       if !path.is_dir() {
           error!(self.log, "Given load directory is malformed, no 'users' subdir, not loading users!");
       } else {
           //self.load_users(txn, &path)?;
       }
       path.pop();
       // =================================================
       Ok(())
   }

   fn load_roles(&mut self, txn: &mut RwTransaction, path: &Path) -> Result<()> {
       for entry in std::fs::read_dir(path)? {
           let entry = entry?;
           let path = entry.path();
           if path.is_file() {
               // will only ever be none if the path has no file name and then how is it a file?!
               let roleID_str = path
                   .file_stem().expect("Found a file with no filename?")
                   .to_str().expect("Found an OsStr that isn't valid Unicode. Fix your OS!");
               let roleID = match u64::from_str_radix(roleID_str, 16) {
                   Ok(i) => i,
                   Err(e) => {
                       warn!(self.log, "File {} had a invalid name. Expected an u64 in [0-9a-z] hex with optional file ending: {}. Skipping!", path.display(), e);
                       continue;
                   }
               };
               let s = match fs::read_to_string(path.as_path()) {
                   Ok(s) => s,
                   Err(e) => {
                       warn!(self.log, "Failed to open file {}: {}, skipping!"
                            , path.display()
                            , e);
                       continue;
                   }
               };
               let role: Role = match toml::from_str(&s) {
                   Ok(r) => r,
                   Err(e) => {
                       warn!(self.log, "Failed to parse role at path {}: {}, skipping!"
                            , path.display()
                            , e);
                       continue;
                   }
               };
               self.put_role(txn, roleID, role)?;
               debug!(self.log, "Loaded role {}", roleID);
           } else {
               warn!(self.log, "Path {} is not a file, skipping!", path.display());
           }
       }

       Ok(())
   }
}

/// This line documents init
pub fn init(log: Logger, config: &Config, env: &lmdb::Environment) -> std::result::Result<PermissionsProvider, crate::error::Error> {
    let mut flags = lmdb::DatabaseFlags::empty();
    flags.set(lmdb::DatabaseFlags::INTEGER_KEY, true);
    let roledb = env.create_db(Some("role"), flags)?;
    debug!(&log, "Opened access database '{}' successfully.", "role");
    let permdb = env.create_db(Some("perm"), flags)?;
    debug!(&log, "Opened access database '{}' successfully.", "perm");
    let userdb = env.create_db(Some("user"), flags)?;
    debug!(&log, "Opened access database '{}' successfully.", "user");
    info!(&log, "Opened all access databases");
    return Ok(PermissionsProvider::new(log, roledb, permdb, userdb));
}

/// A Person, from the Authorization perspective
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct User {
    name: String,

    /// A Person has N ≥ 0 roles.
    /// Persons are only ever given roles, not permissions directly
    roles: Vec<RoleIdentifier>
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
struct Role {
    name: String,

    /// A Role can have parents, inheriting all permissions
    ///
    /// This makes situations where different levels of access are required easier: Each higher
    /// level of access sets the lower levels of access as parent, inheriting their permission; if
    /// you are allowed to manage a machine you are then also allowed to use it and so on
    parents: Vec<RoleIdentifier>,
    permissions: Vec<PermIdentifier>,
}

/// A Permission from the Authorization perspective
///
/// Permissions are rather simple flags. A person can have or not have a permission, dictated by
/// its roles and the permissions assigned to those roles.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct Perm {
    name: String,
}
