use crate::Config;
use std::path::Path;

pub fn read_config_file(path: impl AsRef<Path>) -> Result<Config, serde_dhall::Error> {
    serde_dhall::from_file(path).parse().map_err(Into::into)
}
