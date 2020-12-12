use std::default::Default;
use std::str::FromStr;
use std::path::{Path, PathBuf};
use std::io::Read;
use std::fs;
use std::collections::HashMap;

use serde::{Serialize, Deserialize};

use crate::error::Result;
use crate::machine::MachineDescription;
use crate::db::machine::MachineIdentifier;

pub fn read(path: &Path) -> Result<Config> {
    serde_dhall::from_file(path).parse().map_err(Into::into)
}

#[deprecated]
pub type Settings = Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// A list of address/port pairs to listen on.
    // TODO: This should really be a variant type; that is something that can figure out itself if
    // it contains enough information to open a socket (i.e. it checks if it's a valid path (=>
    // Unix socket) or IPv4/v6 address)
    pub listens: Box<[Listen]>,

    /// Machine descriptions to load
    pub machines: HashMap<MachineIdentifier, MachineDescription>,

    /// Modules to load and their configuration options
    pub modules: HashMap<String, HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Listen {
    pub address: String,
    pub port: Option<u16>,
}

impl Default for Settings {
    fn default() -> Self {
        let modules: HashMap::<String, HashMap<String, String>> = HashMap::new();
        Config {
            listens: Box::new([]),
            machines: HashMap::new(),
            modules: modules,
        }
    }
}

// The default port in the non-assignable i.e. free-use area
pub const DEFAULT_PORT: u16 = 59661;
