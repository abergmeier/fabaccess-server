use tracing_subscriber::{EnvFilter};


use serde::{Serialize, Deserialize};

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

pub fn init(config: &LogConfig) {
    let filter = if let Some(ref filter) = config.filter {
        EnvFilter::new(filter.as_str())
    } else {
        EnvFilter::from_env("BFFH_LOG")
    };

    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter);

    let format = config.format.to_lowercase();
    match format.as_str() {
        "compact" => builder.compact().init(),
        "pretty" => builder.pretty().init(),
        "full" => builder.init(),
        _ => builder.init(),
    }
    tracing::info!(format = format.as_str(), "Logging initialized")
}