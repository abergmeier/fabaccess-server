//!
//!
//!
//! Bastion Executor is NUMA-aware SMP based Fault-tolerant Executor
//!
//! Bastion Executor is a highly-available, fault-tolerant, async communication
//! oriented executor. Bastion's main idea is supplying a fully async runtime
//! with fault-tolerance to work on heavy loads.
//!
//! Main differences between other executors are:
//! * Uses SMP based execution scheme to exploit cache affinity on multiple cores and execution is
//! equally distributed over the system resources, which means utilizing the all system.
//! * Uses NUMA-aware allocation for scheduler's queues and exploit locality on capnp workloads.
//! * Tailored for creating middleware and working with actor model like concurrency and distributed communication.
//!
//! **NOTE:** Bastion Executor is independent of it's framework implementation.
//! It uses [lightproc] to encapsulate and provide fault-tolerance to your future based workloads.
//! You can use your futures with [lightproc] to run your workloads on Bastion Executor without the need to have framework.
//!
//! [lightproc]: https://docs.rs/lightproc
//!

// Force missing implementations
#![warn(missing_docs)]
#![warn(missing_debug_implementations)]
#![warn(unused_imports)]
#![forbid(unused_must_use)]
#![forbid(unused_import_braces)]

pub mod load_balancer;
pub mod placement;
pub mod pool;
pub mod run;
pub mod manage;
mod thread_manager;
mod worker;

///
/// Prelude of Bastion Executor
pub mod prelude {
    pub use crate::pool::*;
}
