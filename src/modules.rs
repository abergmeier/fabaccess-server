//! Indpendent Communication modules
//!
//! This is where dynamic modules are implemented later on using libloading / abi_stable_crates et
//! al.
//! Additionally, FFI modules to other languages (Python/Lua/...) make the most sense in here as
//! well.

mod shelly;
pub use shelly::Shelly;

mod process;
pub use process::Process;
