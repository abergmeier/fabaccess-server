use std::default::Default;
use std::path::{PathBuf};
use std::collections::HashMap;

use serde::{Serialize, Deserialize};






mod dhall;
pub use dhall::read_config_file as read;

use crate::authorization::permissions::{PrivilegesBuf};
use crate::authorization::roles::Role;
use crate::capnp::{Listen, TlsListen};
use crate::logging::LogConfig;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// A description of a machine
///
/// This is the struct that a machine is serialized to/from.
/// Combining this with the actual state of the system will return a machine
pub struct MachineDescription {
    /// The name of the machine. Doesn't need to be unique but is what humans will be presented.
    pub name: String,

    /// An optional description of the Machine.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deser_option")]
    pub description: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deser_option")]
    pub wiki: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deser_option")]
    pub category: Option<String>,

    /// The permission required
    #[serde(flatten)]
    pub privs: PrivilegesBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// A list of address/port pairs to listen on.
    pub listens: Vec<Listen>,

    /// Machine descriptions to load
    pub machines: HashMap<String, MachineDescription>,

    /// Actors to load and their configuration options
    pub actors: HashMap<String, ModuleConfig>,

    /// Initiators to load and their configuration options
    pub initiators: HashMap<String, ModuleConfig>,

    pub mqtt_url: String,

    pub actor_connections: Vec<(String, String)>,
    pub init_connections: Vec<(String, String)>,

    pub db_path: PathBuf,
    pub auditlog_path: PathBuf,

    pub roles: HashMap<String, Role>,

    #[serde(flatten)]
    pub tlsconfig: TlsListen,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tlskeylog: Option<PathBuf>,

    #[serde(default, skip)]
    pub verbosity: isize,

    #[serde(default, skip)]
    pub logging: LogConfig,
}

impl Config {
    pub fn is_quiet(&self) -> bool {
        self.verbosity < 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleConfig {
    pub module: String,
    pub params: HashMap<String, String>
}

pub(crate) fn deser_option<'de, D, T>(d: D) -> std::result::Result<Option<T>, D::Error>
    where D: serde::Deserializer<'de>, T: serde::Deserialize<'de>,
{
    Ok(T::deserialize(d).ok())
}


impl Default for Config {
    fn default() -> Self {
        let mut actors: HashMap::<String, ModuleConfig> = HashMap::new();
        let mut initiators: HashMap::<String, ModuleConfig> = HashMap::new();
        let machines = HashMap::new();

        actors.insert("Actor".to_string(), ModuleConfig {
            module: "Shelly".to_string(),
            params: HashMap::new(),
        });
        initiators.insert("Initiator".to_string(), ModuleConfig {
            module: "TCP-Listen".to_string(),
            params: HashMap::new(),
        });

        Config {
            listens: vec![
                Listen {
                    address: "127.0.0.1".to_string(),
                    port: None,
                }
            ],
            actors,
            initiators,
            machines,
            mqtt_url: "tcp://localhost:1883".to_string(),
            actor_connections: vec![
                ("Testmachine".to_string(), "Actor".to_string()),
            ],
            init_connections: vec![
                ("Initiator".to_string(), "Testmachine".to_string()),
            ],

            db_path: PathBuf::from("/run/bffh/database"),
            auditlog_path: PathBuf::from("/var/log/bffh/audit.log"),
            roles: HashMap::new(),

            tlsconfig: TlsListen {
                certfile: PathBuf::from("./bffh.crt"),
                keyfile: PathBuf::from("./bffh.key"),
                .. Default::default()
            },

            tlskeylog: None,
            verbosity: 0,
            logging: LogConfig::default(),
        }
    }
}
