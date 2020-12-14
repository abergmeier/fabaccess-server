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
    serde_dhall::from_file(path)
        .parse()
        .map_err(Into::into)
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

    /// Actors to load and their configuration options
    pub actors: HashMap<String, ModuleConfig>,

    /// Initiators to load and their configuration options
    pub initiators: HashMap<String, ModuleConfig>,

    pub mqtt_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Listen {
    pub address: String,
    pub port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleConfig {
    pub name: String,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub params: HashMap<String, String>
}

impl Default for Config {
    fn default() -> Self {
        let mut actors: HashMap::<String, ModuleConfig> = HashMap::new();
        let mut initiators: HashMap::<String, ModuleConfig> = HashMap::new();

        actors.insert("Actor".to_string(), ModuleConfig {
            name: "Shelly".to_string(),
            params: HashMap::new(),
        });
        initiators.insert("Initiator".to_string(), ModuleConfig {
            name: "TCP-Listen".to_string(),
            params: HashMap::new(),
        });

        Config {
            listens: Box::new([
                Listen {
                    address: "localhost".to_string(),
                    port: Some(DEFAULT_PORT),
                }
            ]),
            machines: HashMap::new(),
            actors: actors,
            initiators: initiators,
            mqtt_url: "tcp://localhost:1883".to_string(),
        }
    }
}

// The default port in the non-assignable i.e. free-use area
pub const DEFAULT_PORT: u16 = 59661;
