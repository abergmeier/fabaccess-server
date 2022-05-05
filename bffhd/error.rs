use crate::db;
use rsasl::error::SessionError;
use std::fmt;
use std::io;

type DBError = db::Error;

#[derive(Debug)]
/// Shared error type
pub enum Error {
    SASL(SessionError),
    IO(io::Error),
    Boxed(Box<dyn std::error::Error>),
    Capnp(capnp::Error),
    DB(DBError),
    Denied,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::SASL(e) => {
                write!(f, "SASL Error: {}", e)
            }
            Error::IO(e) => {
                write!(f, "IO Error: {}", e)
            }
            Error::Boxed(e) => {
                write!(f, "{}", e)
            }
            Error::Capnp(e) => {
                write!(f, "Cap'n Proto Error: {}", e)
            }
            Error::DB(e) => {
                write!(f, "DB Error: {:?}", e)
            }
            Error::Denied => {
                write!(f, "You do not have the permission required to do that.")
            }
        }
    }
}

impl From<SessionError> for Error {
    fn from(e: SessionError) -> Error {
        Error::SASL(e)
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::IO(e)
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

pub type Result<T> = std::result::Result<T, Error>;
