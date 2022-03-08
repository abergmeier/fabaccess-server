#![forbid(unused_imports, unused_import_braces)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![warn(missing_crate_level_docs)]

//! Diflouroborane
//!
//! This is the capnp component of the FabAccess project.
//! The entry point of bffhd can be found in [bin/bffhd/main.rs](../bin/bffhd/main.rs)

/// Internal Databases build on top of LMDB, a mmap()'ed B-tree DB optimized for reads
pub mod db;

/// Shared error type
pub mod error;

/// Policy decision engine
pub mod permissions;

pub mod users;
pub mod authentication;

/// Resources
pub mod resource;
pub mod resources;

pub mod actors;

pub mod initiators;

pub mod sensors;

pub mod capnp;

pub mod utils;