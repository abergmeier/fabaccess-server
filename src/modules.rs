//! Indpendent Communication modules
//!
//! This is where dynamic modules are implemented later on using libloading / abi_stable_crates et
//! al.
//! Additionally, FFI modules to other languages (Python/Lua/...) make the most sense in here as
//! well.

use slog::Logger;

mod shelly;
pub use shelly::Shelly;

use futures::prelude::*;
use futures::task::Spawn;

use crate::config::Settings;
use crate::error::Result;
