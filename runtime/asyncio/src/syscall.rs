use std::io;
use std::os::unix::prelude::RawFd;
use libc::{c_ulong, c_long};
use crate::ctypes::{IORING_ENTER, IORING_REGISTER_OP};
use super::ctypes::Params;

const ENOMEM: i32 = 12;

const SYS_SETUP: c_long = libc::SYS_io_uring_setup;
const SYS_ENTER: c_long = libc::SYS_io_uring_enter;
const SYS_REGISTER: c_long = libc::SYS_io_uring_register;

/// Syscall io_uring_setup, creating the io_uring ringbuffers
pub fn setup(entries: u32, params: *mut Params) -> io::Result<RawFd> {
    assert!((0 < entries && entries <= 4096), "entries must be between 1 and 4096");
    assert_eq!(entries.count_ones(), 1, "entries must be a power of two");

    let retval = unsafe {
        libc::syscall(SYS_SETUP, entries, params)
    };
    if retval < 0 {
        let err = io::Error::last_os_error();
        if let Some(ENOMEM) = err.raw_os_error() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Failed to lock enough memory. You may need to increase the memlock limit using \
                rlimits"
            ));
        }
        return Err(err);
    } else {
        Ok(retval as RawFd)
    }
}

static_assertions::assert_eq_size!(i64, c_long);

/// enter io_uring, returning when at least `min_complete` events have been completed
pub fn enter(fd: RawFd,
             to_submit: u32,
             min_complete: u32,
             flags: IORING_ENTER,
             args: *const libc::c_void,
             argsz: libc::size_t

) -> io::Result<i64> {
    let retval = unsafe {
        libc::syscall(SYS_ENTER, fd, to_submit, min_complete, flags.bits(), args, argsz)
    };
    if retval < 0 {
        let err = io::Error::last_os_error();
        Err(err)
    } else {
        Ok(retval)
    }
}

/// Register buffers or file descriptors with the kernel for faster usage and not having to use
/// atomics.
pub fn register(fd: RawFd, opcode: IORING_REGISTER_OP, args: *const (), nargs: u32)
    -> io::Result<i64>
{
    let retval = unsafe {
        libc::syscall(SYS_REGISTER, fd, opcode, args, nargs)
    };
    if retval < 0 {
        let err = io::Error::last_os_error();
        Err(err)
    } else {
        Ok(retval)
    }
}