use std::default::Default;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

use serde::{Serialize, Deserialize};

use crate::error::Result;
use crate::db::machine::MachineIdentifier;
use crate::db::access::*;
use crate::machine::MachineDescription;

pub fn read(path: &Path) -> Result<Config> {
    serde_dhall::from_file(path)
        .parse()
        .map_err(Into::into)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorConn {
    pub machine: String,
    pub actor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitiatorConn {
    pub machine: String,
    pub initiator: String,
}

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

    pub actor_connections: Box<[ActorConn]>,
    pub init_connections: Box<[InitiatorConn]>,

    pub db_path: PathBuf,
    pub auditlog_path: PathBuf,

    pub roles: HashMap<String, RoleConfig>,

    /// Path to a certificate chain to be used
    pub certfile: PathBuf,
    pub keyfile: PathBuf,
}


pub(crate) fn deser_option<'de, D, T>(d: D) -> std::result::Result<Option<T>, D::Error>
    where D: serde::Deserializer<'de>, T: serde::Deserialize<'de>,
{
    Ok(T::deserialize(d).ok())
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleConfig {
    #[serde(default = "Vec::new")]
    pub parents: Vec<String>,
    #[serde(default = "Vec::new")]
    pub permissions: Vec<PermRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Listen {
    pub address: String,

    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deser_option")]
    pub port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleConfig {
    pub module: String,
    pub params: HashMap<String, String>
}

impl Default for Config {
    fn default() -> Self {
        let mut actors: HashMap::<String, ModuleConfig> = HashMap::new();
        let mut initiators: HashMap::<String, ModuleConfig> = HashMap::new();
        let mut machines = HashMap::new();

        actors.insert("Actor".to_string(), ModuleConfig {
            module: "Shelly".to_string(),
            params: HashMap::new(),
        });
        initiators.insert("Initiator".to_string(), ModuleConfig {
            module: "TCP-Listen".to_string(),
            params: HashMap::new(),
        });

        machines.insert("Testmachine".to_string(), MachineDescription {
            name: "Testmachine".to_string(),
            description: Some("A test machine".to_string()),
            wiki: None,
            category: None,
            privs: PrivilegesBuf {
                disclose: PermissionBuf::from_string("lab.test.read".to_string()),
                read: PermissionBuf::from_string("lab.test.read".to_string()),
                write: PermissionBuf::from_string("lab.test.write".to_string()),
                manage: PermissionBuf::from_string("lab.test.admin".to_string()),
            },
        });

        Config {
            listens: Box::new([
                Listen {
                    address: "localhost".to_string(),
                    port: Some(DEFAULT_PORT),
                }
            ]),
            machines,
            actors,
            initiators,
            mqtt_url: "tcp://localhost:1883".to_string(),
            actor_connections: Box::new([
                ActorConn { machine: "Testmachine".to_string(), actor: "Actor".to_string() },
            ]),
            init_connections: Box::new([
                InitiatorConn {
                    initiator: "Initiator".to_string(), machine: "Testmachine".to_string()
                },
            ]),

            auditlog_path: PathBuf::from("/var/log/bffh/audit.log"),
            db_path: PathBuf::from("/run/bffh/database"),
            roles: HashMap::new(),

            certfile: PathBuf::from("/etc/bffh/pub.crt"),
            keyfile: PathBuf::from("/etc/bffh/priv.key"),
        }
    }
}

// The default port in the non-assignable i.e. free-use area
pub const DEFAULT_PORT: u16 = 59661;
