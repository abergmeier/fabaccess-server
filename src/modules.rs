//! Indpendent Communication modules
//!
//! This is where dynamic modules are implemented later on using libloading / abi_stable_crates et
//! al.
//! Additionally, FFI modules to other languages (Python/Lua/...) make the most sense in here as
//! well.

use slog::Logger;

mod shelly;

use futures::prelude::*;
use futures::task::Spawn;

use crate::config::Config;
use crate::error::Result;

// spawner is a type that allows 'tasks' to be spawned on it, running them to completion.
pub fn init<S: Spawn>(log: Logger, config: &Config, spawner: &S) -> Result<()> {
    let f = Box::new(shelly::init(log.clone(), config.clone()));
    spawner.spawn_obj(f.into())?;

    Ok(())
}
