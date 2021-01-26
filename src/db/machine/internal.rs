use std::sync::Arc;

use slog::Logger;

use lmdb::{Environment, Transaction, RwTransaction, Cursor};

use super::{MachineIdentifier, MachineState};
use crate::error::Result;

#[derive(Clone, Debug)]
pub struct Internal {
    log: Logger,
    env: Arc<Environment>,
    db: lmdb::Database,
}

impl Internal {
    pub fn new(log: Logger, env: Arc<Environment>, db: lmdb::Database) -> Self {
        Self { log, env, db }
    }

    pub fn get_with_txn<T: Transaction>(&self, txn: &T, id: &String) 
        -> Result<Option<MachineState>> 
    {
        match txn.get(self.db, &id.as_bytes()) {
            Ok(bytes) => {
                let mut machine: MachineState = flexbuffers::from_slice(bytes)?;
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

    pub fn put_with_txn(&self, txn: &mut RwTransaction, uuid: &String, status: &MachineState) 
        -> Result<()>
    {
        let bytes = flexbuffers::to_vec(status)?;
        txn.put(self.db, &uuid.as_bytes(), &bytes, lmdb::WriteFlags::empty())?;

        Ok(())
    }

    pub fn put(&self, id: &MachineIdentifier, status: &MachineState) -> Result<()> {
        let mut txn = self.env.begin_rw_txn()?;
        self.put_with_txn(&mut txn, id, status)?;
        txn.commit().map_err(Into::into)
    }

    pub fn iter<T: Transaction>(&self, txn: &T) -> Result<impl Iterator<Item=MachineState>> {
       let mut cursor = txn.open_ro_cursor(self.db)?;
       Ok(cursor.iter_start().map(|buf| {
           let (kbuf, vbuf) = buf.unwrap();
           flexbuffers::from_slice(vbuf).unwrap()
       }))
    }
}
