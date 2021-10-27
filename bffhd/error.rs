use std::io;
use std::fmt;
use serde_dhall;

use rsasl::SaslError;

use crate::db::DBError;

//FIXME use crate::network;

#[derive(Debug)]
pub enum Error {
    Dhall(serde_dhall::Error),
    SASL(SaslError),
    IO(io::Error),
    Boxed(Box<dyn std::error::Error>),
    Capnp(capnp::Error),
    DB(DBError),
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
            Error::DB(e) => {
                write!(f, "DB Error: {:?}", e)
            },
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

impl From<DBError> for Error {
    fn from(e: DBError) -> Error {
        Error::DB(e)
    }
}

pub(crate) type Result<T> = std::result::Result<T, Error>;