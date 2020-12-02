use slog::Logger;

use crate::config::Settings;
use crate::error::Result;
use crate::db::machine::Status;
use crate::registries::Registries;

use std::pin::Pin;
use futures::prelude::*;
use futures::channel::mpsc;
use futures::ready;
use futures::task::{Poll, Context, Waker, Spawn, FutureObj};
use futures::StreamExt;
use futures_signals::signal::Signal;

use paho_mqtt as mqtt;

// TODO: Late config parsing. Right now the config is validated at the very startup in its
// entirety. This works reasonably enough for this static modules here but if we do dynamic loading
// via dlopen(), lua API, python API etc it will not.
pub async fn run<S: Spawn>(log: Logger, config: Settings, registries: Registries, spawner: S) {
}

