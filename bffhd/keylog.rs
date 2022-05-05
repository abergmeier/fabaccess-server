use std::fmt::Formatter;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::{fmt, io};

// Internal mutable state for KeyLogFile
struct KeyLogFileInner {
    file: File,
    buf: Vec<u8>,
}
impl fmt::Debug for KeyLogFileInner {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.file, f)
    }
}

impl KeyLogFileInner {
    fn new(path: impl AsRef<Path>) -> io::Result<Self> {
        let file = OpenOptions::new().append(true).create(true).open(path)?;

        Ok(Self {
            file,
            buf: Vec::new(),
        })
    }

    fn try_write(&mut self, label: &str, client_random: &[u8], secret: &[u8]) -> io::Result<()> {
        self.buf.truncate(0);
        write!(self.buf, "{} ", label)?;
        for b in client_random.iter() {
            write!(self.buf, "{:02x}", b)?;
        }
        write!(self.buf, " ")?;
        for b in secret.iter() {
            write!(self.buf, "{:02x}", b)?;
        }
        writeln!(self.buf)?;
        self.file.write_all(&self.buf)
    }
}

#[derive(Debug)]
/// [`KeyLog`] implementation that opens a file at the given path
pub struct KeyLogFile(Mutex<KeyLogFileInner>);

impl KeyLogFile {
    /// Makes a new `KeyLogFile`.  The environment variable is
    /// inspected and the named file is opened during this call.
    pub fn new(path: impl AsRef<Path>) -> io::Result<Self> {
        Ok(Self(Mutex::new(KeyLogFileInner::new(path)?)))
    }
}

impl rustls::KeyLog for KeyLogFile {
    fn log(&self, label: &str, client_random: &[u8], secret: &[u8]) {
        match self
            .0
            .lock()
            .unwrap()
            .try_write(label, client_random, secret)
        {
            Ok(()) => {}
            Err(e) => {
                tracing::warn!("error writing to key log file: {}", e);
            }
        }
    }
}
