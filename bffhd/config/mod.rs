use std::path::Path;

use miette::Diagnostic;
use thiserror::Error;

pub(crate) use dhall::deser_option;
pub use dhall::{Config, MachineDescription, ModuleConfig};
mod dhall;

#[derive(Debug, Error, Diagnostic)]
pub enum ConfigError {
    #[error("The config file '{0}' does not exist or is not readable")]
    #[diagnostic(
        code(config::notfound),
        help("Make sure the config file and the directory it's in are readable by the user running bffh")
    )]
    NotFound(String),
    #[error("The path '{0}' does not point to a file")]
    #[diagnostic(
        code(config::notafile),
        help("The config must be a file in the dhall format")
    )]
    NotAFile(String),
    #[error("failed to parse config: {0}")]
    #[diagnostic(code(config::parse))]
    Parse(
        #[from]
        #[source]
        serde_dhall::Error,
    ),
}

pub fn read(file: impl AsRef<Path>) -> Result<Config, ConfigError> {
    let path = file.as_ref();
    if !path.exists() {
        return Err(ConfigError::NotFound(path.to_string_lossy().to_string()));
    }
    if !path.is_file() {
        return Err(ConfigError::NotAFile(path.to_string_lossy().to_string()));
    }
    let mut config = dhall::read_config_file(file)?;
    for (envvar, value) in std::env::vars() {
        match envvar.as_str() {
            // Do things like this?
            // "BFFH_LOG" => config.logging.filter = Some(value),
            _ => {}
        }
    }
    Ok(config)
}
