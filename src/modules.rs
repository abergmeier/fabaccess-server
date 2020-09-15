//! Indpendent Communication modules
//!
//! This is where dynamic modules are implemented later on using libloading / abi_stable_crates et
//! al.
//! Additionally, FFI modules to other languages (Python/Lua/...) make the most sense in here as
//! well.

use slog::Logger;

mod shelly;

use futures::prelude::*;
use futures::task::LocalSpawn;

use crate::config::Settings;
use crate::error::Result;
use crate::registries::Registries;

// spawner is a type that allows 'tasks' to be spawned on it, running them to completion.
pub fn init<S: LocalSpawn>(log: Logger, config: &Settings, spawner: &S, registries: Registries) -> Result<()> {
    let f = Box::new(shelly::run(log.clone(), config.clone(), registries.clone()));
    spawner.spawn_local_obj(f.into())?;

    Ok(())
}
