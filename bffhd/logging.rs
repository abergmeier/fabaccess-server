use tracing_subscriber::EnvFilter;

use serde::{Deserialize, Serialize};
use tracing_subscriber::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Log filter string in the tracing format `target[span{field=value}]=level`.
    /// lvalue is optional and multiple filters can be combined with comma.
    /// e.g. `warn,diflouroborane::actors=debug` will only print `WARN` and `ERROR` unless the
    /// message is logged in a span below `diflouroborane::actors` (i.e. by an actor task) in
    /// which case `DEBUG` and `INFO` will also be printed.
    pub filter: Option<String>,

    pub format: String,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            filter: None,
            format: "full".to_string(),
        }
    }
}

pub fn init(config: &LogConfig) -> console::Server {
    let (console, server) = console::ConsoleLayer::new();

    let filter = if let Some(ref filter) = config.filter {
        EnvFilter::new(filter.as_str())
    } else {
        EnvFilter::from_env("BFFH_LOG")
    };

    let format = &config.format;
    // TODO: Restore output format settings being settable
    let fmt_layer = tracing_subscriber::fmt::layer().with_filter(filter);

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(console)
        .init();

    tracing::info!(format = format.as_str(), "Logging initialized");

    server
}
