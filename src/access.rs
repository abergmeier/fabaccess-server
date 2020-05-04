//! Access control logic
//!

use slog::Logger;

use crate::config::Config;


pub struct PermissionsProvider {
    log: Logger,
}

impl PermissionsProvider {
    pub fn new(log: Logger) -> Self {
        Self { log }
    }
}

/// This line documents init
pub async fn init(log: Logger, config: &Config) -> std::result::Result<PermissionsProvider, Box<dyn std::error::Error>> {
    return Ok(PermissionsProvider::new(log));
}
