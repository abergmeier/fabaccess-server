mod actuators;

pub use actuators::Actuator;

#[derive(Clone)]
/// BFFH registries
///
/// This struct is only a reference to the underlying registries - cloning it will generate a new
/// reference, not clone the registries
pub struct Registries {
    actuators: actuators::Actuators,
}
