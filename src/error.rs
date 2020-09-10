use std::io;
use std::fmt;
use toml;

use rsasl::SaslError;

#[derive(Debug)]
pub enum Error {
    TomlDe(toml::de::Error),
    TomlSer(toml::ser::Error),
    SASL(SaslError),
    IO(io::Error),
    Boxed(Box<dyn std::error::Error>),
    Capnp(capnp::Error),
    LMDB(lmdb::Error),
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

pub type Result<T> = std::result::Result<T, Error>;
