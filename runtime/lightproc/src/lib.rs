//!
//!
//! LightProc is Lightweight Process abstraction for Rust.
//!
//! Beneath the implementation:
//! * It uses futures with lifecycle callbacks to implement Erlang like processes.
//! * Contains basic pid(process id) to identify processes.
//! * All panics inside futures are propagated to upper layers.
//!
//! The naming convention of this crate comes from [Erlang's Lightweight Processes].
//!
//! [Erlang's Lightweight Processes]: https://en.wikipedia.org/wiki/Light-weight_process
//!

// Force missing implementations
#![forbid(missing_docs)]
#![forbid(missing_debug_implementations)]
#![forbid(unused_import_braces)]
#![warn(unused_imports)]
#![forbid(unused_must_use)]
//TODO: reenable #![forbid(unused_variables)]

mod catch_unwind;
mod layout_helpers;
mod proc_data;
mod proc_ext;
mod proc_layout;
mod proc_vtable;
mod raw_proc;
mod state;

pub mod lightproc;
pub mod proc_handle;
pub mod recoverable_handle;

pub use proc_data::GroupId;

/// The lightproc prelude.
///
/// The prelude re-exports lightproc structs and handles from this crate.
pub mod prelude {
    pub use crate::lightproc::*;
    pub use crate::proc_handle::*;
    pub use crate::recoverable_handle::*;
}
