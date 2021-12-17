use std::collections::HashSet;
use std::sync::Arc;
use lmdb::{RwTransaction, Transaction};
use crate::db::{RawDB, DB, AllocAdapter, Environment, Result};
use crate::db::{DatabaseFlags, LMDBorrow, RoTransaction, WriteFlags, };
use super::User;

use rkyv::{Deserialize, Archived};

type Adapter = AllocAdapter<User>;
#[derive(Clone, Debug)]
pub struct UserDB {
    env: Arc<Environment>,
    db: DB<Adapter>,
}

impl UserDB {
    pub unsafe fn new(env: Arc<Environment>, db: RawDB) -> Self {
        let db = DB::new_unchecked(db);
        Self { env, db }
    }

    pub unsafe fn open(env: Arc<Environment>) -> Result<Self> {
        let db = RawDB::open(&env, Some("user"))?;
        Ok(Self::new(env, db))
    }

    pub unsafe fn create(env: Arc<Environment>) -> Result<Self> {
        let flags = DatabaseFlags::empty();
        let db = RawDB::create(&env, Some("user"), flags)?;
        Ok(Self::new(env, db))
    }

    pub fn get(&self, uid: &str) -> Result<Option<LMDBorrow<RoTransaction, Archived<User>>>> {
        let txn = self.env.begin_ro_txn()?;
        if let Some(state) = self.db.get(&txn, &uid.as_bytes())? {
            let ptr = state.into();
            Ok(Some(unsafe { LMDBorrow::new(ptr, txn) }))
        } else {
            Ok(None)
        }
    }

    pub fn put(&self, uid: &str, user: &User) -> Result<()> {
        let mut txn = self.env.begin_rw_txn()?;
        let flags = WriteFlags::empty();
        self.db.put(&mut txn, &uid.as_bytes(), user, flags)?;
        Ok(())
    }

    pub fn get_all(&self) -> Result<Vec<(String, User)>> {
        let txn = self.env.begin_ro_txn()?;
        let mut cursor = self.db.open_ro_cursor(&txn)?;
        let iter = cursor.iter_start();
        let mut out = Vec::new();
        let mut deserializer = rkyv::Infallible;
        for user in iter {
            let (uid, user) = user?;
            let uid = unsafe { std::str::from_utf8_unchecked(uid).to_string() };
            let user: User = user.deserialize(&mut deserializer).unwrap();
            out.push((uid, user));
        }

        Ok(out)
    }
}

pub struct UserIndex {
    env: Arc<Environment>,
    usernames: RawDB,
    roles: RawDB,
}

impl UserIndex {
    pub fn update(&self, old: &User, new: &User) -> Result<()> {
        assert_eq!(old.id, new.id);
        let mut txn = self.env.begin_rw_txn()?;
        if old.username != new.username {
            self.update_username(&mut txn, new.id, &old.username, &new.username)?;
        }

        let mut to_remove: HashSet<&String> = old.roles.iter().collect();
        let mut to_add: HashSet<&String> = HashSet::new();
        for role in new.roles.iter() {
            // If a role wasn't found in the old ones it's a new one that's being added
            if !to_remove.remove(role) {
                to_add.insert(role);
            }
            // Otherwise it's in both sets so we just ignore it.
        }

        self.update_roles(&mut txn, new.id, to_remove, to_add)?;
        txn.commit()?;
        Ok(())
    }

    fn update_username(&self, txn: &mut RwTransaction, uid: u128, old: &String, new: &String)
        -> Result<()>
    {
        let flags = WriteFlags::empty();
        self.usernames.del(txn, &old.as_bytes(), Some(&uid.to_ne_bytes()))?;
        self.usernames.put(txn, &new.as_bytes(), &uid.to_ne_bytes(), flags)?;
        Ok(())
    }

    fn update_roles(&self,
                    txn: &mut RwTransaction,
                    uid: u128,
                    remove: HashSet<&String>,
                    add: HashSet<&String>
        ) -> Result<()>
    {
        let flags = WriteFlags::empty();
        for role in remove.iter() {
            self.roles.del(txn, &role.as_bytes(), Some(&uid.to_ne_bytes()))?;
        }
        for role in add.iter() {
            self.roles.put(txn, &role.as_bytes(), &uid.to_ne_bytes(), flags)?;
        }
        Ok(())
    }
}