use tracing_subscriber::{EnvFilter};
use crate::Config;

pub fn init(config: &Config) {
    let builder = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env());
    let format = config.log_format.to_lowercase();
    match format.as_str() {
        "compact" => builder.compact().init(),
        "pretty" => builder.pretty().init(),
        "full" => builder.init(),
        _ => builder.init(),
    }

    tracing::info!(format = format.as_str(), "Logging initialized")
}