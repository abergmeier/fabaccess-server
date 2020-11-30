use std::sync::Arc;

use crate::db::machine::MachineDB;

mod actuators;
mod sensors;

pub use actuators::{Actuator, ActBox, StatusSignal};

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
    pub fn new(db: Arc<MachineDB>) -> Self {
        Registries {
            actuators: actuators::Actuators::new(),
            sensors: sensors::Sensors::new(db),
        }
    }
}
