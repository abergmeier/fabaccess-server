mod actuators;
mod sensors;

pub use actuators::{Actuator, ActBox};

#[derive(Clone)]
/// BFFH registries
///
/// This struct is only a reference to the underlying registries - cloning it will generate a new
/// reference, not clone the registries
pub struct Registries {
    pub actuators: actuators::Actuators,
}

impl Registries {
    pub fn new() -> Self {
        Registries {
            actuators: actuators::Actuators::new()
        }
    }
}
