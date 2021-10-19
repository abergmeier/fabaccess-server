// FIXME: No.
#![allow(dead_code)]
#![forbid(unused_imports)]

//mod modules;
//mod log;
//mod config;
//mod connection;
//mod machine;
//mod builtin;
//mod server;
//mod actor;
//mod initiator;
mod space;

mod resource;
mod schema;
mod state;
mod db;
mod network;
pub mod oid;
mod varint;
mod error;

/*

use clap::{App, Arg};

use std::io;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;

use std::sync::Arc;

use smol::Executor;

use error::Error;

use slog::Logger;

use paho_mqtt::AsyncClient;
use crate::config::Config;
*/
