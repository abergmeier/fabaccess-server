#![forbid(unused_imports)]
#![warn(missing_debug_implementations)]

//! Diflouroborane
//!
//! This is the server component of the FabAccess project.
//! The entry point of bffhd can be found in [bin/bffhd/main.rs](../bin/bffhd/main.rs)
//!
//! P.S.: If you're curious about the name; the project was initially called "Better Fablab
//! Friend and Helper" (BFFH). And the chemical formula of Diflouroborane is BF2H.

/// Internal Databases build on top of LMDB, a mmap()'ed B-tree DB optimized for reads
pub mod db;

/// Shared error type
pub mod error;

/// Policy decision engine
pub mod permissions;

pub mod users;

/// Resources
pub mod resource;
pub mod resources;

pub mod server;

pub mod utils;