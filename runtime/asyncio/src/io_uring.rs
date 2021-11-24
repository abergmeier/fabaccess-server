use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::io;
use std::marker::PhantomData;
use std::mem::{size_of, align_of};
use std::ops::Deref;
use std::sync::atomic::{AtomicU32, Ordering};
use std::os::unix::prelude::RawFd;
use std::pin::Pin;
use std::ptr::NonNull;
use std::sync::Arc;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use nix::sys::{mman, mman::{MapFlags, ProtFlags}};
use crate::completion::Completion;
use crate::cq::CQ;
use crate::cqe::{CQE, CQEs};
use crate::ctypes::{CQOffsets, IORING_ENTER, SQOffsets};
use crate::sq::SQ;
use crate::sqe::{SQE, SQEs};
use super::ctypes::{Params, io_uring_sqe, IORING_CQ, IORING_FEAT,
                    IORING_OFF_CQ_RING, IORING_OFF_SQ_RING, IORING_OFF_SQES, IORING_SQ};
use super::syscall;

#[derive(Debug)]
pub struct IoUring {
    fd: RawFd,
    params: Params,
    sq: SQ,
    cq: CQ,
}

unsafe fn mmap(map_size: usize, fd: RawFd, offset: i64) -> nix::Result<*mut libc::c_void> {
    mman::mmap(
        std::ptr::null_mut(),
        map_size,
        ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
        MapFlags::MAP_SHARED | MapFlags::MAP_POPULATE,
        fd,
        offset
    )
}

impl IoUring {
    pub fn setup(entries: u32) -> io::Result<Self> {
        let mut params = Params::default();
        let fd = syscall::setup(entries, &mut params)?;

        let mut sq_map_size = (params.sq_off.array as usize) +
            (params.sq_entries as usize) * size_of::<u32>();
        let mut cq_map_size = (params.cq_off.cqes as usize) +
            (params.cq_entries as usize) * size_of::<CQE>();

        // If we can use a single mmap() syscall to map sq, cq and cqe the size of the total map
        // is the largest of `sq_map_size` and `cq_map_size`.
        if params.features.contains(IORING_FEAT::SINGLE_MMAP) {
            sq_map_size = sq_map_size.max(cq_map_size);
            cq_map_size = sq_map_size;
        }

        let sq_ptr = unsafe {
            mmap(sq_map_size as usize, fd, IORING_OFF_SQ_RING as i64)?
        };

        let sqes_map_size = (params.sq_entries as usize) * size_of::<io_uring_sqe>();
        let sqes = unsafe {
            let ptr = mmap(sqes_map_size, fd, IORING_OFF_SQES as i64)?.cast();
            std::slice::from_raw_parts_mut(ptr, params.sq_entries as usize)
        };

        let sq = unsafe {
            SQ::new(sq_ptr,
                    params.sq_off,
                    sqes,
                    sq_map_size,
                    sqes_map_size
            )
        };

        let cq_ptr = if params.features.contains(IORING_FEAT::SINGLE_MMAP) {
            sq_ptr
        } else {
            unsafe {
                mmap(cq_map_size, fd, IORING_OFF_CQ_RING as i64)?
            }
        };
        let cq = unsafe {
            CQ::new(cq_ptr,
                    params.cq_off,
                    params.cq_entries,
                    sq_ptr != cq_ptr,
                    cq_map_size,
            )
        };

        Ok(IoUring {
            fd,
            params,
            sq,
            cq,
        })
    }

    pub fn try_prepare(
        &self,
        count: u32,
        prepare: impl FnOnce(SQEs<'_>)
    ) -> Option<()> {
        self.handle_completions();
        if !self.cq.try_reserve(count) {
            return None;
        }

        if let Some(sqes) = self.sq.try_reserve(count) {
            let start = sqes.start();
            prepare(sqes);
            self.sq.prepare(start, count);
            Some(())
        } else {
            None
        }
    }

    pub fn poll_prepare<'cx>(
        mut self: Pin<&mut Self>,
        ctx: &mut Context<'cx>,
        count: u32,
        prepare: impl for<'sq> FnOnce(SQEs<'sq>, &mut Context<'cx>) -> Completion
    ) -> Poll<Completion> {
        Pin::new(&mut self.sq).poll_prepare(ctx, count, prepare)
    }

    pub fn poll_submit<'cx>(
        mut self: Pin<&mut Self>,
        ctx: &mut Context<'cx>,
        head: u32,
    ) -> Poll<()> {
        let fd = self.fd;
        Pin::new(&mut self.sq).poll_submit(ctx, fd, head)
    }

    pub fn submit_wait(&self) -> io::Result<u32> {
        self.sq.submit_wait(self.fd)
    }

    pub fn handle_completions(&self) {
        self.cq.handle(|cqe| {
            let udata = cqe.user_data;
            if udata != 0 {
                let completion = unsafe {
                    Completion::from_raw(udata)
                };
                completion.complete(cqe.result())
            }
        });
    }
}

impl Future for &IoUring {
    type Output = io::Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.handle_completions();
        match self.sq.submit(self.fd, Some(cx.waker())) {
            Ok(_) => Poll::Pending,
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}