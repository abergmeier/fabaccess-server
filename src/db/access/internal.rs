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

use crate::db::access::{Permission, Role, RoleIdentifier, RoleDB};
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
    pub fn _check<T: Transaction, P: AsRef<Permission>>(&self, txn: &T, user: &User, perm: &P)
        -> Result<bool>
    {
        // Tally all roles. Makes dependent roles easier
        let mut roles = HashSet::new();
        for roleID in user.roles.iter() {
            self._tally_role(txn, &mut roles, roleID)?;
        }

        // Iter all unique role->permissions we've found and early return on match. 
        // TODO: Change this for negative permissions?
        for role in roles.iter() {
            for perm_rule in role.permissions.iter() {
                if perm_rule.match_perm(perm) {
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
       // TODO implement this for the new format
       unimplemented!()
   }

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
       if path.is_file() {
           let roles = Role::load_file(path)?;

           for (k,v) in roles.iter() {
               self.put_role(txn, k, v.clone())?;
           }
       } else {
           for entry in std::fs::read_dir(path)? {
               let roles = Role::load_file(entry?.path())?;

               for (k,v) in roles.iter() {
                   self.put_role(txn, k, v.clone())?;
               }
           }
       }

       Ok(())
   }
}

impl RoleDB for Internal {
    fn check<P: AsRef<Permission>>(&self, user: &User, perm: &P) -> Result<bool> {
        let txn = self.env.begin_ro_txn()?;
        self._check(&txn, user, perm)
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
