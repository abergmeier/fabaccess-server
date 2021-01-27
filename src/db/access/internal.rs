use std::collections::HashMap;

use std::path::Path;
use std::sync::Arc;

use flexbuffers;

use slog::Logger;
use lmdb::{Environment, Transaction, RwTransaction, Cursor};

use crate::config::Config;
use crate::error::Result;

use crate::db::access::{Permission, Role, RoleIdentifier, RoleDB};
use crate::db::user::UserData;

#[derive(Clone, Debug)]
pub struct Internal {
    log: Logger,
    env: Arc<Environment>,
    roledb: lmdb::Database,
}

impl Internal {
    pub fn new(log: Logger, env: Arc<Environment>, roledb: lmdb::Database) -> Self {
        Self { log, env, roledb, }
    }

    /// Check if a given user has the given permission
    #[allow(unused)]
    pub fn _check<T: Transaction, P: AsRef<Permission>>(&self, txn: &T, user: &UserData, perm: &P)
        -> Result<bool>
    {
        debug!(self.log, "Checking user {:?} for permission {:?}", user, perm.as_ref());
        // Tally all roles. Makes dependent roles easier
        let mut roles = HashMap::new();
        for role_id in user.roles.iter() {
            debug!(self.log, "Tallying role {} for its parents", role_id);
            self._tally_role(txn, &mut roles, role_id)?;
        }

        // Iter all unique role->permissions we've found and early return on match. 
        // TODO: Change this for negative permissions?
        for (roleid, role) in roles.iter() {
            debug!(self.log, "  checking role {}", roleid);
            for perm_rule in role.permissions.iter() {
                if perm_rule.match_perm(perm) {
                    debug!(self.log, "  matches permission rule {}", perm_rule);
                    return Ok(true);
                }
                trace!(self.log, "  rejecting permission rule {}", perm_rule);
            }
        }

        debug!(self.log, "Checked all roles, rejecting access");

        return Ok(false);
    }

    fn _tally_role<T: Transaction>(&self, txn: &T, roles: &mut HashMap<RoleIdentifier, Role>, role_id: &RoleIdentifier) -> Result<()> {
        if let Some(role) = self._get_role(txn, role_id)? {
            // Only check and tally parents of a role at the role itself if it's the first time we
            // see it
            if !roles.contains_key(&role_id) {
                for parent in role.parents.iter() {
                    self._tally_role(txn, roles, parent)?;
                }

                roles.insert(role_id.clone(), role);
            }
        } else {
            info!(self.log, "Did not find role {} while trying to tally", role_id);
        }

        Ok(())
    }

    pub fn _get_role<'txn, T: Transaction>(&self, txn: &'txn T, role_id: &RoleIdentifier) -> Result<Option<Role>> {
        let string = format!("{}", role_id);
        debug!(self.log, "Reading role '{}'", &string);
        match txn.get(self.roledb, &string.as_bytes()) {
            Ok(bytes) => {
                Ok(Some(flexbuffers::from_slice(bytes)?))
            },
            Err(lmdb::Error::NotFound) => { Ok(None) },
            Err(e) => { Err(e.into()) }
        }
    }

    fn put_role(&self, txn: &mut RwTransaction, role_id: &RoleIdentifier, role: Role) -> Result<()> {
        let bytes = flexbuffers::to_vec(role)?;
        let string = format!("{}", role_id);
        txn.put(self.roledb, &string.as_bytes(), &bytes, lmdb::WriteFlags::empty())?;

        Ok(())
    }


    pub fn dump_roles(&self) -> Result<Vec<(RoleIdentifier, Role)>> {
        let txn = self.env.begin_ro_txn()?;
        self.dump_roles_txn(&txn)
    }
    pub fn dump_roles_txn<T: Transaction>(&self, txn: &T) -> Result<Vec<(RoleIdentifier, Role)>> {
        let mut cursor = txn.open_ro_cursor(self.roledb)?;

        let mut vec = Vec::new();
        for r in cursor.iter_start() {
            match r {
                Ok( (k,v) ) => {
                    let role_id_str = unsafe { std::str::from_utf8_unchecked(k) };
                    let role_id = role_id_str.parse::<RoleIdentifier>().unwrap();
                    match flexbuffers::from_slice(v) {
                        Ok(role) => vec.push((role_id, role)),
                        Err(e) => error!(self.log, "Bad format for roleid {}: {}", role_id, e),
                    }
                },
                Err(e) => return Err(e.into()),
            }
        }

        Ok(vec)
    }

    pub fn load_roles<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let mut txn = self.env.begin_rw_txn()?;
        self.load_roles_txn(&mut txn, path.as_ref())?;

        // In case the above didn't error, commit.
        txn.commit();
        Ok(())
    }
    fn load_roles_txn(&self, txn: &mut RwTransaction, path: &Path) -> Result<()> {
        let roles = Role::load_file(path)?;

        for (k,v) in roles.iter() {
            self.put_role(txn, k, v.clone())?;
        }

        debug!(self.log, "Loaded roles: {:?}", roles);

        Ok(())
    }
}

impl RoleDB for Internal {
    fn get_type_name(&self) -> &'static str {
        "Internal"
    }

    fn check(&self, user: &UserData, perm: &Permission) -> Result<bool> {
        let txn = self.env.begin_ro_txn()?;
        self._check(&txn, user, &perm)
    }

    fn get_role(&self, role_id: &RoleIdentifier) -> Result<Option<Role>> {
        let txn = self.env.begin_ro_txn()?;
        self._get_role(&txn, role_id)
    }

    fn tally_role(&self, roles: &mut HashMap<RoleIdentifier, Role>, role_id: &RoleIdentifier) -> Result<()> {
        let txn = self.env.begin_ro_txn()?;
        self._tally_role(&txn, roles, role_id)
    }
}



/// Initialize the access db by loading all the lmdb databases
pub fn init(log: Logger, _config: &Config, env: Arc<lmdb::Environment>) 
    -> std::result::Result<Internal, crate::error::Error> 
{
    let mut flags = lmdb::DatabaseFlags::empty();
    flags.set(lmdb::DatabaseFlags::INTEGER_KEY, true);
    let roledb = env.create_db(Some("role"), flags)?;
    debug!(&log, "Opened access database '{}' successfully.", "role");
    //let permdb = env.create_db(Some("perm"), flags)?;
    //debug!(&log, "Opened access database '{}' successfully.", "perm");

    Ok(Internal::new(log, env, roledb))
}
