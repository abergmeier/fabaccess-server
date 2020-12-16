use std::sync::Arc;
use std::path::PathBuf;
use std::str::FromStr;

use slog::Logger;

use crate::error::Result;
use crate::config::Settings;

/// (Hashed) password database
pub mod pass;

/// User storage
pub mod user;

/// Access control storage
///
/// Stores&Retrieves Permissions and Roles
pub mod access;

/// Machine storage
///
/// Stores&Retrieves Machines
pub mod machine;

#[derive(Clone)]
pub struct Databases {
    pub access: Arc<access::AccessControl>,
    pub machine: Arc<machine::internal::Internal>,
    pub passdb: Arc<pass::PassDB>,
    pub userdb: Arc<user::Internal>,
}

const LMDB_MAX_DB: u32 = 16;

impl Databases {
    pub fn new(log: &Logger, config: &Settings) -> Result<Self> {

        // Initialize the LMDB environment. This blocks until the mmap() finishes
        info!(log, "LMDB env");
        let env = lmdb::Environment::new()
            .set_flags(lmdb::EnvironmentFlags::MAP_ASYNC | lmdb::EnvironmentFlags::NO_SUB_DIR)
            .set_max_dbs(LMDB_MAX_DB as libc::c_uint)
            .open(&PathBuf::from_str("/tmp/a.db").unwrap())?;

        // Start loading the machine database, authentication system and permission system
        // All of those get a custom logger so the source of a log message can be better traced and
        // filtered
        let env = Arc::new(env);
        let mdb = machine::init(log.new(o!("system" => "machines")), &config, env.clone())?;

        let permdb = access::init(log.new(o!("system" => "permissions")), &config, env.clone())?;
        let mut ac = access::AccessControl::new(permdb);

        let passdb = pass::PassDB::init(log.new(o!("system" => "passwords")), env.clone()).unwrap();

        let userdb = user::init(log.new(o!("system" => "users")), &config, env.clone())?;

        Ok(Self {
            access: Arc::new(ac),
            passdb: Arc::new(passdb),
            machine: Arc::new(mdb),
            userdb: Arc::new(userdb),
        })
    }
}
