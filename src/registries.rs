mod actuators;
mod sensors;

pub use actuators::{Actuator, ActBox, StatusSignal};
pub use sensors::{Sensor, SensBox};

#[derive(Clone)]
/// BFFH registries
///
/// This struct is only a reference to the underlying registries - cloning it will generate a new
/// reference, not clone the registries
pub struct Registries {
    pub actuators: actuators::Actuators,
    pub sensors: sensors::Sensors,
}

impl Registries {
    pub fn new() -> Self {
        Registries {
            actuators: actuators::Actuators::new(),
            sensors: sensors::Sensors::new(),
        }
    }
}
