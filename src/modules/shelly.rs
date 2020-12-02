use slog::Logger;

use crate::config::Settings;
use crate::error::Result;
use crate::db::machine::Status;

use std::pin::Pin;
use futures::prelude::*;
use futures::channel::mpsc;
use futures::ready;
use futures::task::{Poll, Context, Waker, Spawn, FutureObj};
use futures::StreamExt;
use futures_signals::signal::Signal;

use paho_mqtt as mqtt;

