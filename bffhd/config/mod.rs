mod dhall;
pub use dhall::read_config_file as read;
pub use dhall::{Config, ModuleConfig, MachineDescription};
pub(crate) use dhall::deser_option;

struct ConfigBuilder;
impl ConfigBuilder {

}

