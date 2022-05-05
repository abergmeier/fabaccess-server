use std::fmt::Formatter;
use std::net::ToSocketAddrs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::deser_option;

#[derive(Debug, Clone, Serialize, Deserialize)]
/// API Socket Configuration block.
///
/// One configuration block can result in several sockets if the given `address` resolves to more
/// than one SocketAddr. BFFH will attempt to bind to all of them.
pub struct Listen {
    pub address: String,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deser_option"
    )]
    pub port: Option<u16>,
}

impl Listen {
    pub fn to_tuple(&self) -> (&str, u16) {
        (self.address.as_str(), self.port.unwrap_or(DEFAULT_PORT))
    }
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TlsListen {
    pub certfile: PathBuf,
    pub keyfile: PathBuf,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ciphers: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_min_version: Option<String>,
    #[serde(default = "Vec::new", skip_serializing_if = "Vec::is_empty")]
    pub protocols: Vec<String>,
}

// The default port in the non-assignable i.e. free-use area
pub const DEFAULT_PORT: u16 = 59661;
