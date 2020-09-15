use std::str::FromStr;
use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};
use std::io::Read;
use std::fs::File;

use crate::error::Result;

use std::default::Default;

use std::collections::HashMap;

use config::Config;
pub use config::ConfigError;
use glob::glob;

pub fn read(path: &Path) -> Result<Settings> {
    let mut settings = Config::default();
    settings
        .merge(config::File::from(path)).unwrap();

    Ok(settings.try_into()?)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub listens: Box<[Listen]>,
    pub shelly: Option<ShellyCfg>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellyCfg {
    pub mqtt_url: String
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Listen {
    pub address: String,
    pub port: Option<u16>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            listens: Box::new([Listen {
                    address: "127.0.0.1".to_string(),
                    port: Some(DEFAULT_PORT)
                },
                Listen {
                    address: "::1".to_string(),
                    port: Some(DEFAULT_PORT)
                }]),
            shelly: Some(ShellyCfg {
                mqtt_url: "127.0.0.1:1883".to_string()
            }),
        }
    }
}

// The default port in the non-assignable i.e. free-use area
pub const DEFAULT_PORT: u16 = 59661;
