use std::io;
use std::fmt;
use serde_dhall;

use rsasl::SaslError;

// SpawnError is a somewhat ambigous name, `use as` to make it futures::SpawnError instead.
use futures::task as futures_task;

use paho_mqtt::errors as mqtt;

//FIXME use crate::network;

#[derive(Debug)]
pub enum Error {
    Dhall(serde_dhall::Error),
    SASL(SaslError),
    IO(io::Error),
    Boxed(Box<dyn std::error::Error>),
    Capnp(capnp::Error),
    LMDB(lmdb::Error),
    FuturesSpawn(futures_task::SpawnError),
    MQTT(mqtt::Error),
    BadVersion((u32,u32)),
    Denied,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Dhall(e) => {
                write!(f, "Dhall coding error: {}", e)
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
            Error::FuturesSpawn(e) => {
                write!(f, "Future could not be spawned: {}", e)
            },
            Error::MQTT(e) => {
                write!(f, "Paho MQTT encountered an error: {}", e)
            },
            Error::BadVersion((major,minor)) => {
                write!(f, "Peer uses API version {}.{} which is incompatible!", major, minor)
            }
            Error::Denied => {
                write!(f, "You do not have the permission required to do that.")
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

impl From<serde_dhall::Error> for Error {
    fn from(e: serde_dhall::Error) -> Error {
        Error::Dhall(e)
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

impl From<futures_task::SpawnError> for Error {
    fn from(e: futures_task::SpawnError) -> Error {
        Error::FuturesSpawn(e)
    }
}

impl From<mqtt::Error> for Error {
    fn from(e: mqtt::Error) -> Error {
        Error::MQTT(e)
    }
}

/*impl From<network::Error> for Error {
    fn from(e: network::Error) -> Error {
        Error::EventNetwork(e)
    }
}*/

pub(crate) type Result<T> = std::result::Result<T, Error>;
