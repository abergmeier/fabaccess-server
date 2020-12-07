use std::io;
use std::fmt;
use toml;

use rsasl::SaslError;

// SpawnError is a somewhat ambigous name, `use as` to make it futures::SpawnError instead.
use futures::task as futures_task;

use paho_mqtt::errors as mqtt;

use crate::network;

#[derive(Debug)]
pub enum Error {
    TomlDe(toml::de::Error),
    TomlSer(toml::ser::Error),
    SASL(SaslError),
    IO(io::Error),
    Boxed(Box<dyn std::error::Error>),
    Capnp(capnp::Error),
    LMDB(lmdb::Error),
    FlexbuffersDe(flexbuffers::DeserializationError),
    FlexbuffersSer(flexbuffers::SerializationError),
    FuturesSpawn(futures_task::SpawnError),
    MQTT(mqtt::Error),
    Config(config::ConfigError),
    BadVersion((u32,u32)),
    Argon2(argon2::Error),
    EventNetwork(network::Error),
    Denied,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::TomlDe(e) => {
                write!(f, "TOML Decoding error: {}", e)
            },
            Error::TomlSer(e) => {
                write!(f, "TOML Serialization error: {}", e)
            },
            Error::SASL(e) => {
                write!(f, "SASL Error: {}", e)
            },
            Error::IO(e) => {
                write!(f, "IO Error: {}", e)
            },
            Error::Boxed(e) => {
                write!(f, "{}", e)
            },
            Error::Capnp(e) => {
                write!(f, "Cap'n Proto Error: {}", e)
            },
            Error::LMDB(e) => {
                write!(f, "LMDB Error: {}", e)
            },
            Error::FlexbuffersDe(e) => {
                write!(f, "Flexbuffers decoding error: {}", e)
            },
            Error::FlexbuffersSer(e) => {
                write!(f, "Flexbuffers encoding error: {}", e)
            },
            Error::FuturesSpawn(e) => {
                write!(f, "Future could not be spawned: {}", e)
            },
            Error::MQTT(e) => {
                write!(f, "Paho MQTT encountered an error: {}", e)
            },
            Error::Config(e) => {
                write!(f, "Failed to parse config: {}", e)
            }
            Error::Argon2(e) => {
                write!(f, "Argon2 en/decoding failure: {}", e)
            }
            Error::BadVersion((major,minor)) => {
                write!(f, "Peer uses API version {}.{} which is incompatible!", major, minor)
            }
            Error::Denied => {
                write!(f, "You do not have the permission required to do that.")
            }
            Error::EventNetwork(e) => {
                e.fmt(f)
            }
        }
    }
}

impl From<SaslError> for Error {
    fn from(e: SaslError) -> Error {
        Error::SASL(e)
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::IO(e)
    }
}

impl From<toml::de::Error> for Error {
    fn from(e: toml::de::Error) -> Error {
        Error::TomlDe(e)
    }
}

impl From<toml::ser::Error> for Error {
    fn from(e: toml::ser::Error) -> Error {
        Error::TomlSer(e)
    }
}

impl From<Box<dyn std::error::Error>> for Error {
    fn from(e: Box<dyn std::error::Error>) -> Error {
        Error::Boxed(e)
    }
}

impl From<capnp::Error> for Error {
    fn from(e: capnp::Error) -> Error {
        Error::Capnp(e)
    }
}

impl From<lmdb::Error> for Error {
    fn from(e: lmdb::Error) -> Error {
        Error::LMDB(e)
    }
}

impl From<flexbuffers::DeserializationError> for Error {
    fn from(e: flexbuffers::DeserializationError) -> Error {
        Error::FlexbuffersDe(e)
    }
}

impl From<flexbuffers::SerializationError> for Error {
    fn from(e: flexbuffers::SerializationError) -> Error {
        Error::FlexbuffersSer(e)
    }
}

impl From<futures::SpawnError> for Error {
    fn from(e: futures::SpawnError) -> Error {
        Error::FuturesSpawn(e)
    }
}

impl From<mqtt::Error> for Error {
    fn from(e: mqtt::Error) -> Error {
        Error::MQTT(e)
    }
}

impl From<config::ConfigError> for Error {
    fn from(e: config::ConfigError) -> Error {
        Error::Config(e)
    }
}

impl From<network::Error> for Error {
    fn from(e: network::Error) -> Error {
        Error::EventNetwork(e)
    }
}

impl From<argon2::Error> for Error {
    fn from(e: argon2::Error) -> Error {
        Error::Argon2(e)
    }
}

pub(crate) type Result<T> = std::result::Result<T, Error>;
