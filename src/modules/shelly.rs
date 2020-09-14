use slog::Logger;
use crate::config::Config;

use crate::registries::Actuator;

pub async fn init(log: Logger, config: Config) {
    info!(log, "This is where shelly support would start up, IF IT WAS IMPLEMENTED");
}

/// An actuator for all Shellies connected listening on one MQTT broker
///
/// This actuator can power toggle an arbitrariy named shelly on the broker it is connected to. If
/// you need to toggle shellies on multiple brokers you need multiple instanced of this actuator.
struct Shelly {
    
}

impl Actuator for Shelly {

}
