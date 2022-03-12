use std::fs::{File, OpenOptions};
use std::io;
use std::io::{LineWriter, Write};
use std::sync::Mutex;

use crate::Config;
use serde::{Serialize, Deserialize};
use serde_json::Serializer;

#[derive(Debug)]
pub struct AuditLog {
    writer: Mutex<LineWriter<File>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogLine {
    timestamp: i64,
    machine: String,
    state: String,
}

impl AuditLog {
    pub fn new(config: &Config) -> io::Result<Self> {
        let fd = OpenOptions::new().create(true).append(true).open(&config.auditlog_path)?;
        let writer = Mutex::new(LineWriter::new(fd));
        Ok(Self { writer })
    }

    pub fn log(&self, machine: &str, state: &str) -> io::Result<()> {
        let timestamp = chrono::Utc::now().timestamp();
        let line = AuditLogLine { timestamp, machine: machine.to_string(), state: state.to_string() };

        let mut guard = self.writer.lock().unwrap();
        let mut writer: &mut LineWriter<File> = &mut *guard;

        let mut ser = Serializer::new(&mut writer);
        line.serialize(&mut ser).expect("failed to serialize audit log line");
        writer.write("\n".as_bytes())?;
        Ok(())
    }
}
