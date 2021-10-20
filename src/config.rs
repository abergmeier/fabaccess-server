use std::default::Default;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

use serde::{Serialize, Deserialize, Deserializer, Serializer};

use crate::error::Result;
use std::fmt::Formatter;
use std::net::{SocketAddr, IpAddr, ToSocketAddrs};
use std::str::FromStr;
use crate::permissions::{PermRule, RoleIdentifier};
use serde::de::Error;

pub fn read(path: &Path) -> Result<Config> {
    serde_dhall::from_file(path)
        .parse()
        .map_err(Into::into)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// A list of address/port pairs to listen on.
    // TODO: This should really be a variant type; that is something that can figure out itself if
    // it contains enough information to open a socket (i.e. it checks if it's a valid path (=>
    // Unix socket) or IPv4/v6 address)
    pub listens: Vec<Listen>,

    /// Machine descriptions to load
    //pub machines: HashMap<MachineIdentifier, MachineDescription>,

    /// Actors to load and their configuration options
    pub actors: HashMap<String, ModuleConfig>,

    /// Initiators to load and their configuration options
    pub initiators: HashMap<String, ModuleConfig>,

    pub mqtt_url: String,

    pub actor_connections: Box<[(String, String)]>,
    pub init_connections: Box<[(String, String)]>,

    pub db_path: PathBuf,

    pub roles: HashMap<RoleIdentifier, RoleConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleConfig {
    #[serde(default = "Vec::new")]
    pub parents: Vec<RoleIdentifier>,
    #[serde(default = "Vec::new")]
    pub permissions: Vec<PermRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleConfig {
    pub module: String,
    pub params: HashMap<String, String>
}

#[derive(Debug, Clone)]
pub struct Listen {
    address: String,
    port: Option<u16>,
}

impl std::fmt::Display for Listen {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", &self.address, self.port.unwrap_or(DEFAULT_PORT))
    }
}

impl ToSocketAddrs for Listen {
    type Iter = <(String, u16) as ToSocketAddrs>::Iter;

    fn to_socket_addrs(&self) -> std::io::Result<Self::Iter> {
        if let Some(port) = self.port {
            (self.address.as_str(), port).to_socket_addrs()
        } else {
            (self.address.as_str(), DEFAULT_PORT).to_socket_addrs()
        }
    }
}

impl<'de> serde::Deserialize<'de> for Listen {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        deserializer.deserialize_str(ListenVisitor)
    }
}
impl serde::Serialize for Listen {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
        where S: Serializer
    {
        if let Some(port) = self.port {
            serializer.serialize_str(&format!("{}:{}", self.address, port))
        } else {
            serializer.serialize_str(&self.address)
        }
    }
}

struct ListenVisitor;
impl<'de> serde::de::Visitor<'de> for ListenVisitor {
    type Value = Listen;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "A string encoding a valid IP or Hostname (e.g. 127.0.0.1 or [::1]) with \
        or without a defined port")
    }

    fn visit_str<E>(self, v: &str) -> std::result::Result<Self::Value, E>
        where E: Error
    {
        let sockaddr = SocketAddr::from_str(v);
        if let Ok(address) = sockaddr {
            return Ok(Listen {
                address: address.ip().to_string(),
                port: Some(address.port()),
            })
        }

        let ipaddr = IpAddr::from_str(v);
        if let Ok(address) = ipaddr {
            return Ok(Listen {
                address: address.to_string(),
                port: None,
            })
        }

        let mut split = v.split(':');
        let address = split.next()
            .expect("str::split should always return at least one element")
            .to_string();
        let port = if let Some(port) = split.next() {
            let port: u16 = port.parse()
                .map_err(|_| {
                    E::custom(&format!("Expected valid ip address or hostname with or without \
                    port. Failed to parse \"{}\".", v))
                })?;

            Some(port)
        } else {
            None
        };

        Ok(Listen { address, port })
    }
}

impl Default for Config {
    fn default() -> Self {
        let mut actors: HashMap::<String, ModuleConfig> = HashMap::new();
        let mut initiators: HashMap::<String, ModuleConfig> = HashMap::new();

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
            mqtt_url: "tcp://localhost:1883".to_string(),
            actor_connections: Box::new([
                ("Testmachine".to_string(), "Actor".to_string()),
            ]),
            init_connections: Box::new([
                ("Initiator".to_string(), "Testmachine".to_string()),
            ]),

            db_path: PathBuf::from("/run/bffh/database"),
            roles: HashMap::new(),
        }
    }
}

// The default port in the non-assignable i.e. free-use area
pub const DEFAULT_PORT: u16 = 59661;
