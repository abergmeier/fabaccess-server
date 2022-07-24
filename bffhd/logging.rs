use std::path::Path;
use tracing_subscriber::{EnvFilter, reload};
use serde::{Deserialize, Serialize};
use tracing_subscriber::fmt::format::Format;
use tracing_subscriber::prelude::*;
use tracing_subscriber::reload::Handle;

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

pub enum LogOutput<'a> {
    Journald,
    Stdout,
    File(&'a Path),
}
pub struct LogConfig2<'a, F> {
    output: LogOutput<'a>,
    filter_str: Option<&'a str>,
    format: Format<F>
}

pub fn init(config: &LogConfig) -> console::Server {
    let subscriber = tracing_subscriber::registry();

    let (console_layer, server) = console::ConsoleLayer::new();
    let subscriber = subscriber.with(console_layer);

    let filter = if let Some(ref filter) = config.filter {
        EnvFilter::new(filter.as_str())
    } else {
        EnvFilter::from_env("BFFH_LOG")
    };

    let format = config.format.to_lowercase();

    let fmt_layer = tracing_subscriber::fmt::layer();

    match format.as_ref() {
        "pretty" => {
            let fmt_layer = fmt_layer
                .pretty()
                .with_filter(filter);
            subscriber.with(fmt_layer).init();
        }
        "compact" => {
            let fmt_layer = fmt_layer
                .compact()
                .with_filter(filter);
            subscriber.with(fmt_layer).init();
        }
        _ => {
            let fmt_layer = fmt_layer
                .with_filter(filter);
            subscriber.with(fmt_layer).init();
        }
    }

    tracing::info!(format = format.as_str(), "Logging initialized");

    server
}
