use std::fmt::{Debug, Formatter};
use std::io;
use std::marker::PhantomData;
use std::mem::{size_of, align_of};
use std::sync::atomic::{AtomicU32, Ordering};
use std::os::unix::prelude::RawFd;
use std::pin::Pin;
use std::ptr::NonNull;
use std::task::{Context, Poll, Waker};
use crossbeam_queue::SegQueue;
use nix::sys::{mman, mman::{MapFlags, ProtFlags}};
use crate::completion::Completion;
use crate::cq::CQ;
use crate::cqe::{CQE, CQEs};
use crate::ctypes::{CQOffsets, IORING_ENTER, SQOffsets};
use crate::sq::SQ;
use crate::sqe::SQEs;
use super::ctypes::{Params, io_uring_sqe, IORING_CQ, IORING_FEAT,
                    IORING_OFF_CQ_RING, IORING_OFF_SQ_RING, IORING_OFF_SQES, IORING_SQ};
use super::syscall;

#[derive(Debug)]
pub struct IoUring {
    fd: RawFd,
    params: Params,
    sq: SQ,
    cq: CQ,

    waiting: SegQueue<(u32, Waker)>,
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
        println!("{:?} {}", params.sq_off, sq_map_size);

        // If we can use a single mmap() syscall to map sq, cq and cqe the size of the total map
        // is the largest of `sq_map_size` and `cq_map_size`.
        if params.features.contains(IORING_FEAT::SINGLE_MMAP) {
            sq_map_size = sq_map_size.max(cq_map_size);
            cq_map_size = sq_map_size;
        }

        println!("{:?}", params.cq_off);
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
            waiting: SegQueue::new(),
        })
    }

    pub fn try_prepare<'cx>(
        &self,
        count: u32,
        prepare: impl FnOnce(SQEs<'_>)
    ) -> Option<()> {
        // TODO: Lock the required amount of slots on both submission and completion queue, then
        //       construct the sqes.
        if let Some(sqes) = self.sq.try_reserve(count) {
            Some(prepare(sqes))
        } else {
            None
        }
    }

    pub fn submit(&self) -> io::Result<u32> {
        self.sq.submit(self.fd)
    }

    pub fn cqes(&self) -> CQEs {
        CQEs::new(&self.cq)
    }
}