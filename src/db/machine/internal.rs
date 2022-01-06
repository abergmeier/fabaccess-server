use std::sync::Arc;

use slog::Logger;

use lmdb::{Environment, Transaction, RwTransaction, Cursor, RoTransaction};
use crate::audit::AuditLog;

use super::{MachineIdentifier, MachineState};
use crate::error::Result;

#[derive(Debug)]
pub struct Internal {
    log: Logger,
    audit: AuditLog,
    env: Arc<Environment>,
    db: lmdb::Database,
}

impl Internal {
    pub fn new(log: Logger, audit: AuditLog, env: Arc<Environment>, db: lmdb::Database) -> Self {
        Self { log, audit, env, db }
    }

    pub fn get_with_txn<T: Transaction>(&self, txn: &T, id: &String) 
        -> Result<Option<MachineState>> 
    {
        match txn.get(self.db, &id.as_bytes()) {
            Ok(bytes) => {
                let machine: MachineState = flexbuffers::from_slice(bytes)?;
                Ok(Some(machine))
            },
            Err(lmdb::Error::NotFound) => { Ok(None) },
            Err(e) => { Err(e.into()) },
        }
    }

    pub fn get(&self, id: &MachineIdentifier) -> Result<Option<MachineState>> {
        let txn = self.env.begin_ro_txn()?;
        self.get_with_txn(&txn, id)
    }

    pub fn put_with_txn(&self, txn: &mut RwTransaction, id: &String, status: &MachineState)
        -> Result<()>
    {
        let bytes = flexbuffers::to_vec(status)?;
        txn.put(self.db, &id.as_bytes(), &bytes, lmdb::WriteFlags::empty())?;

        Ok(())
    }

    pub fn put(&self, id: &MachineIdentifier, status: &MachineState) -> Result<()> {
        self.audit.log(id, status)?;
        let mut txn = self.env.begin_rw_txn()?;
        self.put_with_txn(&mut txn, id, status)?;
        txn.commit().map_err(Into::into)
    }

    pub fn iter<'txn, T: Transaction>(&self, txn: &'txn T)
        -> Result<impl Iterator<Item=(&'txn str, MachineState)>>
    {
       let mut cursor = txn.open_ro_cursor(self.db)?;
       Ok(cursor.iter_start().map(|buf| {
           let (kbuf, vbuf) = buf.unwrap();
           let id = unsafe { std::str::from_utf8_unchecked(kbuf) };
           let state = flexbuffers::from_slice(vbuf).unwrap();
           (id, state)
       }))
    }

    pub fn txn(&self) -> Result<RoTransaction> {
        let txn = self.env.begin_ro_txn()?;
        Ok(txn)
    }
}
