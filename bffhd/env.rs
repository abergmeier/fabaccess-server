//! Information extracted from the (build) environment
//!

/// The BFFH version, as an UTF-8 string
///
/// This is of the format "<major>.<minor>.<patch>" if build as a normal release
/// or "<major> .<minor>.<patch>-<commit hash>" if built from source
pub const VERSION: &'static str = env!("BFFHD_VERSION_STRING");
