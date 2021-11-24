use std::cell::Cell;
use std::io::IoSliceMut;
use std::os::unix::prelude::RawFd;
use std::pin::Pin;
use std::task::{Context, Poll};
use futures_io::AsyncRead;
use crate::completion::Completion;
use crate::ctypes::IORING_OP;
use crate::io_uring::IoUring;
use crate::sqe::{SQE, SQEs};
use crate::submission::Submission;

pub struct File {
    fd: RawFd,
    submission: Submission,
}

impl File {
    pub fn new(fd: RawFd, io_uring: &'static IoUring) -> Self {
        Self { fd, submission: Submission::new(io_uring) }
    }

    fn prepare_read<'sq>(
        fd: RawFd,
        buf: &mut [u8],
        sqes: &mut SQEs<'sq>,
    ) -> SQE<'sq>
    {
        let mut sqe = sqes.next().expect("prepare_read requires at least one SQE");
        sqe.set_opcode(IORING_OP::READ);
        sqe.set_address(buf.as_ptr() as u64);
        sqe.set_fd(fd);
        sqe.set_len(buf.len() as i32);
        sqe
    }
}

impl AsyncRead for File {
    fn poll_read(mut self: Pin<&mut Self>, ctx: &mut Context<'_>, buf: &mut [u8])
        -> Poll<std::io::Result<usize>>
    {
        let fd = self.fd;
        Pin::new(&mut self.submission).poll(ctx, 1, |sqes| {
            Self::prepare_read(fd, buf, sqes)
        }).map(|res| res.map(|val| val as usize))
    }
}