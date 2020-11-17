use std::collections::HashSet;

use std::convert::TryInto;

use std::path::{Path, PathBuf};
use std::fs;
use std::io::Write;
use std::sync::Arc;

use flexbuffers;
use serde::{Serialize, Deserialize};

use slog::Logger;
use lmdb::{Environment, Transaction, RwTransaction, Cursor};

use crate::config::Settings;
use crate::error::Result;

use crate::db::access::{PermIdentifier, Role, RoleIdentifier, RoleDB};
use crate::db::user::{UserIdentifier, User};

#[derive(Clone, Debug)]
pub struct Internal {
    log: Logger,
    env: Arc<Environment>,
    roledb: lmdb::Database,
    userdb: lmdb::Database,
}

impl Internal {
    pub fn new(log: Logger, env: Arc<Environment>, roledb: lmdb::Database, userdb: lmdb::Database) -> Self {
        Self { log, env, roledb, userdb }
    }

    /// Check if a given user has the given permission
    #[allow(unused)]
    pub fn _check<T: Transaction>(&self, txn: &T, user: &User, permID: &PermIdentifier) -> Result<bool> {
        // Tally all roles. Makes dependent roles easier
        let mut roles = HashSet::new();
        for roleID in user.roles.iter() {
            self._tally_role(txn, &mut roles, roleID)?;
        }

        // Iter all unique role->permissions we've found and early return on match. 
        // TODO: Change this for negative permissions?
        for role in roles.iter() {
            for perm in role.permissions.iter() {
                if permID == perm {
                    return Ok(true);
                }
            }
        }

        return Ok(false);
    }

    fn _tally_role<T: Transaction>(&self, txn: &T, roles: &mut HashSet<Role>, roleID: &RoleIdentifier) -> Result<()> {
        if let Some(role) = self._get_role(txn, roleID)? {
            // Only check and tally parents of a role at the role itself if it's the first time we
            // see it
            if !roles.contains(&role) {
                for parent in role.parents.iter() {
                    self._tally_role(txn, roles, parent)?;
                }

                roles.insert(role);
            }
        }

        Ok(())
    }

    pub fn _get_role<'txn, T: Transaction>(&self, txn: &'txn T, roleID: &RoleIdentifier) -> Result<Option<Role>> {
        let string = format!("{}", roleID);
        match txn.get(self.roledb, &string.as_bytes()) {
            Ok(bytes) => {
                Ok(Some(flexbuffers::from_slice(bytes)?))
            },
            Err(lmdb::Error::NotFound) => { Ok(None) },
            Err(e) => { Err(e.into()) }
        }
    }

   fn put_role(&self, txn: &mut RwTransaction, roleID: &RoleIdentifier, role: Role) -> Result<()> {
       let bytes = flexbuffers::to_vec(role)?;
       let string = format!("{}", roleID);
       txn.put(self.roledb, &string.as_bytes(), &bytes, lmdb::WriteFlags::empty())?;

       Ok(())
   }

   pub fn dump_db<T: Transaction>(&mut self, txn: &T, mut path: PathBuf) -> Result<()> {
       path.push("roles");
       let mut k = Ok(());
       if !path.is_dir() {
           k = fs::create_dir(&path);
       }
       if let Err(e) = k {
          error!(self.log, "Failed to create 'roles' directory: {}, skipping!", e);
          return Ok(())
       } else {
           // Rust's stdlib considers the last element the file name even when it's a directory so
           // we have to put a dummy here for .set_filename() to work correctly
           path.push("dummy");
           self.dump_roles(txn, path.clone())?;
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
           let out = toml::to_vec(&role)?;
           fp.write_all(&out)?;
       }

       Ok(())
   }

   //fn dump_perms<T: Transaction>(&mut self, txn: &T, mut path: PathBuf) -> Result<()> {
   //    let mut perm_cursor = txn.open_ro_cursor(self.permdb)?;
   //    for buf in perm_cursor.iter_start() {
   //        let (kbuf, vbuf) = buf?;
   //        let (kbytes, _rest) = kbuf.split_at(std::mem::size_of::<u64>());
   //        let permID = u64::from_ne_bytes(kbytes.try_into().unwrap());
   //        let perm: Perm = flexbuffers::from_slice(vbuf)?;
   //        let filename = format!("{:x}.toml", permID);
   //        path.set_file_name(filename);
   //        let mut fp = std::fs::File::create(&path)?;
   //        let out = toml::to_vec(&perm)?;
   //        fp.write_all(&out)?;
   //    }

   //    Ok(())
   //}

   pub fn load_db(&mut self, txn: &mut RwTransaction, mut path: PathBuf) -> Result<()> {
       path.push("roles");
       if !path.is_dir() {
           error!(self.log, "Given load directory is malformed, no 'roles' subdir, not loading roles!");
       } else {
           self.load_roles(txn, path.as_path())?;
       }

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
               let roleID = match str::parse(roleID_str) {
                   Ok(i) => i,
                   Err(e) => {
                       warn!(self.log, "File {} had a invalid name.", path.display());
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
               self.put_role(txn, &roleID, role)?;
               debug!(self.log, "Loaded role {}", &roleID);
           } else {
               warn!(self.log, "Path {} is not a file, skipping!", path.display());
           }
       }

       Ok(())
   }
}

impl RoleDB for Internal {
    fn check(&self, user: &User, permID: &PermIdentifier) -> Result<bool> {
        let txn = self.env.begin_ro_txn()?;
        self._check(&txn, user, permID)
    }

    fn get_role(&self, roleID: &RoleIdentifier) -> Result<Option<Role>> {
        let txn = self.env.begin_ro_txn()?;
        self._get_role(&txn, roleID)
    }

    fn tally_role(&self, roles: &mut HashSet<Role>, roleID: &RoleIdentifier) -> Result<()> {
        let txn = self.env.begin_ro_txn()?;
        self._tally_role(&txn, roles, roleID)
    }
}



/// Initialize the access db by loading all the lmdb databases
pub fn init(log: Logger, config: &Settings, env: Arc<lmdb::Environment>) 
    -> std::result::Result<Internal, crate::error::Error> 
{
    let mut flags = lmdb::DatabaseFlags::empty();
    flags.set(lmdb::DatabaseFlags::INTEGER_KEY, true);
    let roledb = env.create_db(Some("role"), flags)?;
    debug!(&log, "Opened access database '{}' successfully.", "role");
    //let permdb = env.create_db(Some("perm"), flags)?;
    //debug!(&log, "Opened access database '{}' successfully.", "perm");
    let userdb = env.create_db(Some("user"), flags)?;
    debug!(&log, "Opened access database '{}' successfully.", "user");
    info!(&log, "Opened all access databases");

    Ok(Internal::new(log, env, roledb, userdb))
}
