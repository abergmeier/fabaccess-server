use std::sync::Arc;
use std::fs;
use std::io::Write;
use std::str::FromStr;
use std::path::PathBuf;

use slog::Logger;
use uuid::Uuid;

use lmdb::{Environment, Transaction, RwTransaction, Cursor};

use futures::{Future, Stream, StreamExt};
use futures::stream;
use futures::future::Ready;
use futures::stream::Iter;

use super::{MachineIdentifier, MachineState, MachineDB};
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

    pub fn get<T: Transaction>(&self, txn: &T, uuid: &Uuid) 
        -> Result<Option<MachineState>> 
    {
        match txn.get(self.db, uuid.as_bytes()) {
            Ok(bytes) => {
                let mut machine: Machine = flexbuffers::from_slice(bytes)?;
                machine.id = *uuid;

                Ok(Some(machine))
            },
            Err(lmdb::Error::NotFound) => { Ok(None) },
            Err(e) => { Err(e.into()) },
        }
    }

    pub fn put(&self, txn: &mut RwTransaction, uuid: &Uuid, status: MachineStatus) 
        -> Result<()>
    {
        let bytes = flexbuffers::to_vec(status)?;
        txn.put(self.db, uuid.as_bytes(), &bytes, lmdb::WriteFlags::empty())?;

        Ok(())
    }

    pub fn load_db(&mut self, txn: &mut RwTransaction, mut path: PathBuf) -> Result<()> {
       path.push("machines");
       for entry in std::fs::read_dir(path)? {
           let entry = entry?;
           let path = entry.path();
           if path.is_file() {
               // will only ever be none if the path has no file name and then how is it a file?!
               let machID_str = path
                   .file_stem().expect("Found a file with no filename?")
                   .to_str().expect("Found an OsStr that isn't valid Unicode. Fix your OS!");
               let machID = match uuid::Uuid::from_str(machID_str) {
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
               let mach: MachineState = match toml::from_str(&s) {
                   Ok(r) => r,
                   Err(e) => {
                       warn!(self.log, "Failed to parse mach at path {}: {}, skipping!"
                            , path.display()
                            , e);
                       continue;
                   }
               };
               self.put(txn, &machID, mach)?;
               debug!(self.log, "Loaded machine {}", machID);
           } else {
               warn!(self.log, "Path {} is not a file, skipping!", path.display());
           }
       }

       Ok(())
    }

    pub fn dump_db<T: Transaction>(&self, txn: &T, mut path: PathBuf) -> Result<()> {
        path.push("machines");
       let mut mach_cursor = txn.open_ro_cursor(self.db)?;
       for buf in mach_cursor.iter_start() {
           let (kbuf, vbuf) = buf?;
           let machID = uuid::Uuid::from_slice(kbuf).unwrap();
           let mach: MachineState = flexbuffers::from_slice(vbuf)?;
           let filename = format!("{}.yml", machID.to_hyphenated().to_string());
           path.set_file_name(filename);
           let mut fp = std::fs::File::create(&path)?;
           let out = toml::to_vec(&mach)?;
           fp.write_all(&out)?;
       }

       Ok(())
    }

    pub fn iter<T: Transaction>(&self, txn: &T) -> _ {
       let mut cursor = txn.open_ro_cursor(self.db)?;
       Ok(cursor.iter_start().map(|buf| {
           let (kbuf, vbuf) = buf.unwrap();
           let machID = uuid::Uuid::from_slice(kbuf).unwrap();
           flexbuffers::from_slice(vbuf).unwrap()
       }))
    }
}

impl MachineDB for Internal {
    fn get_status(&self, machID: &MachineIdentifier) -> Ready<Result<Option<MachineState>>> {
        let txn = self.env.begin_ro_txn().unwrap();
        futures::future::ready(self.get(&txn, machID))
    }

    fn put_status(&self, machID: &MachineIdentifier, machine: MachineState) -> Ready<Result<()>> {
        let mut txn = self.env.begin_rw_txn().unwrap();
        self.put(&mut txn, machID, machine).unwrap();
        txn.commit().unwrap();

        futures::future::ready(Ok(()))
    }

    fn iter_status(&self) -> _ {
        let txn = self.env.begin_ro_txn().unwrap();
        stream::iter(self.iter(&txn))
    }
}
