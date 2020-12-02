use std::sync::Arc;

pub mod actuators;
pub mod sensors;

#[derive(Clone)]
/// BFFH registries
///
/// This struct is only a reference to the underlying registries - cloning it will generate a new
/// reference, not clone the registries
pub struct Registries {
    pub sensors: sensors::Sensors,
}

impl Registries {
    pub fn new() -> Self {
        Registries {
            sensors: sensors::Sensors::new(),
        }
    }
}
