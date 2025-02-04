use miette::Diagnostic;
use once_cell::sync::OnceCell;
use std::fs::{File, OpenOptions};
use std::io;
use std::io::{LineWriter, Write};
use std::sync::Mutex;
use thiserror::Error;

use crate::Config;
use serde::{Deserialize, Serialize};
use serde_json::Serializer;

pub static AUDIT: OnceCell<AuditLog> = OnceCell::new();

// TODO: Make the audit log a tracing layer
#[derive(Debug)]
pub struct AuditLog {
    writer: Mutex<LineWriter<File>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogLine<'a> {
    timestamp: i64,
    machine: &'a str,
    state: &'a str,
}

#[derive(Debug, Error, Diagnostic)]
#[error(transparent)]
#[repr(transparent)]
pub struct Error(#[from] pub io::Error);

impl AuditLog {
    pub fn new(config: &Config) -> Result<&'static Self, Error> {
        AUDIT.get_or_try_init(|| {
            tracing::debug!(path = %config.auditlog_path.display(), "Initializing audit log");
            let fd = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&config.auditlog_path)?;
            let writer = Mutex::new(LineWriter::new(fd));
            Ok(Self { writer })
        })
    }

    pub fn log(&self, machine: &str, state: &str) -> io::Result<()> {
        let timestamp = chrono::Utc::now().timestamp();
        let line = AuditLogLine {
            timestamp,
            machine,
            state,
        };

        tracing::debug!(?line, "writing audit log line");

        let mut guard = self.writer.lock().unwrap();
        let mut writer: &mut LineWriter<File> = &mut *guard;

        let mut ser = Serializer::new(&mut writer);
        line.serialize(&mut ser)
            .expect("failed to serialize audit log line");
        writer.write_all("\n".as_bytes())?;
        Ok(())
    }
}
