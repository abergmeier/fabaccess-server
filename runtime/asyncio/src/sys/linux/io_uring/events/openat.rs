use std::ffi::CString;
use std::mem::ManuallyDrop;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::prelude::RawFd;
use std::path::Path;
use iou::{SQE, SQEs};
use iou::sqe::{Mode, OFlag};
use crate::sys::linux::io_uring::cancellation::Cancellation;
use super::Event;

pub struct OpenAt {
    pub path: CString,
    pub dir_fd: RawFd,
    pub flags: OFlag,
    pub mode: Mode,
}

impl OpenAt {
    pub fn without_dir(path: impl AsRef<Path>, flags: OFlag, mode: Mode) -> Self {
        let path = CString::new(path.as_ref().as_os_str().as_bytes()).unwrap();
        Self { path, dir_fd: libc::AT_FDCWD, flags, mode }
    }
}

impl Event for OpenAt {
    fn sqes_needed() -> u32 {
        1
    }

    unsafe fn prepare<'a>(&mut self, sqs: &mut SQEs<'a>) -> SQE<'a> {
        let mut sqe = sqs.single().unwrap();
        sqe.prep_openat(self.dir_fd, &*self.path, self.flags, self.mode);
        sqe
    }

    fn cancel(this: ManuallyDrop<Self>) -> Cancellation where Self: Sized {
        ManuallyDrop::into_inner(this).path.into()
    }
}