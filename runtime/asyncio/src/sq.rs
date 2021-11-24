use std::cell::{Cell, UnsafeCell};
use std::fmt::{Debug, Formatter};
use std::io;
use std::mem::ManuallyDrop;
use std::os::unix::prelude::RawFd;
use std::pin::Pin;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU32, compiler_fence, fence, Ordering};
use std::task::{Context, Poll, Waker};
use crossbeam_queue::SegQueue;
use nix::sys::mman::munmap;
use crate::completion::Completion;
use crate::ctypes::{IORING_ENTER, IORING_SQ, io_uring_sqe, SQOffsets};
use crate::sqe::{SQE, SQEs};
use crate::syscall;

pub struct SQ {
    /// Head of the submission queue. This value is set by the kernel when it consumes SQE.
    /// Thus we need to use atomic operations when passing information, making sure both the kernel
    /// and program have a consistent view of its contents.
    array_head: &'static AtomicU32,

    /// The head of the sqes buffer. This value is our local cache of `array_head` that's not
    /// shared with or modified by the kernel. We use it to index the start of the prepared SQE.
    /// This means that this value lags behind after `array_head`.
    sqes_head: UnsafeCell<u32>,

    /// Tail of the submission queue. While this will be modified by the userspace program only,
    /// the kernel uses atomic operations to read it so we want to use atomic operations to write
    /// it.
    array_tail: &'static AtomicU32,
    // non-atomic cache of array_tail
    cached_tail: UnsafeCell<u32>,
    /// Tail of the sqes buffer. This value serves as our local cache of `array_tail` and, in
    /// combination with `sqes_head` allows us to more efficiently submit SQE by skipping already
    /// submitted ones.
    /// `sqes_tail` marks the end of the prepared SQE.
    sqes_tail: UnsafeCell<u32>,

    ring_mask: u32,
    num_entries: u32,

    flags: &'static AtomicU32,

    dropped: &'static  AtomicU32,

    array: &'static [AtomicU32],
    sqes: &'static mut [UnsafeCell<io_uring_sqe>],

    sq_ptr: NonNull<()>,
    sq_map_size: usize,
    sqes_map_size: usize,

    /// Queue of tasks waiting for a submission, either because they need free slots or because
    waiters: SegQueue<Waker>,
    submitter: Cell<Option<Waker>>,
}

static_assertions::assert_not_impl_any!(SQ: Send, Sync);

impl Drop for SQ {
    fn drop(&mut self) {
        unsafe {
            munmap(self.sq_ptr.as_ptr().cast(), self.sq_map_size);
            let sqes_ptr: *mut libc::c_void = self.sqes.as_mut_ptr().cast();
            munmap(sqes_ptr, self.sqes_map_size);
        }
    }
}

impl Debug for SQ {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        unsafe {
            // TODO: Complete
            f.debug_struct("SQ")
             .field("head", self.array_head)
             .field("tail", self.array_tail)
             .field("ring_mask", &self.ring_mask)
             .field("num_entries", &self.num_entries)
             .field("flags", self.flags)
             .field("dropped", self.dropped)
             .field("array", &self.array)
             .finish()
        }
    }
}

impl SQ {
    pub unsafe fn new(ptr: *mut libc::c_void,
                      offs: SQOffsets,
                      sqes: &'static mut [UnsafeCell<io_uring_sqe>],
                      sq_map_size: usize,
                      sqes_map_size: usize,
    ) -> Self {
        // Sanity check the pointer and offsets. If these fail we were probably passed an
        // offsets from an uninitialized parameter struct.
        assert!(!ptr.is_null());
        assert_ne!(offs.head, offs.tail);

        // Eagerly extract static values. Since they won't ever change again there's no reason to
        // not read them now.
        let ring_mask = *(ptr.offset(offs.ring_mask as isize).cast());
        let num_entries = *(ptr.offset(offs.ring_entries as isize).cast());

        // These are valid Rust references; they are valid for the entire lifetime of self,
        // properly initialized by the kernel and well aligned.
        let array_head: &AtomicU32 = &*(ptr.offset(offs.head as isize).cast());
        let sqes_head = UnsafeCell::new(array_head.load(Ordering::Acquire));
        let array_tail: &AtomicU32 = &*ptr.offset(offs.tail as isize).cast();
        let sqes_tail = UnsafeCell::new(array_tail.load(Ordering::Acquire));
        let cached_tail = UnsafeCell::new(array_tail.load(Ordering::Acquire));
        let flags = &*ptr.offset(offs.flags as isize).cast();
        let dropped = &*ptr.offset(offs.dropped as isize).cast();

        let array = std::slice::from_raw_parts(
            ptr.offset(offs.array as isize).cast(),
            sqes.len() as usize,
        );
        let sq_ptr = NonNull::new_unchecked(ptr).cast();

        Self {
            array_head,
            sqes_head,
            array_tail,
            sqes_tail,
            cached_tail,
            ring_mask,
            num_entries,
            flags,
            dropped,
            array,
            sqes,
            sq_ptr,
            sq_map_size,
            sqes_map_size,
            waiters: SegQueue::new(),
            submitter: Cell::new(None),
        }
    }

    #[inline(always)]
    fn sqes_head(&self) -> &mut u32 {
        unsafe { &mut *self.sqes_head.get() }
    }

    #[inline(always)]
    fn sqes_tail(&self) -> &mut u32 {
        unsafe { &mut *self.sqes_tail.get() }
    }

    #[inline(always)]
    fn cached_tail(&self) -> &mut u32 {
        unsafe { &mut *self.cached_tail.get() }
    }

    #[inline(always)]
    fn increment_tail(&self, count: u32) -> u32 {
        let tail = self.sqes_tail();
        let old = *tail;
        *tail = (*tail).wrapping_add(count);
        old
    }

    #[inline(always)]
    fn increment_head(&self, count: u32) -> u32{
        let head = self.sqes_head();
        let old = *head;
        *head = (*head).wrapping_add(count);
        old
    }

    #[inline(always)]
    fn used(&self) -> u32 {
        (*self.sqes_tail()).wrapping_sub(*self.sqes_head())
    }

    #[inline(always)]
    fn available(&self) -> u32 {
        self.num_entries - self.used()
    }

    #[inline(always)]
    fn to_submit(&self) -> u32 {
        let shared_tail = self.array_tail.load(Ordering::Relaxed);
        let cached_tail = *self.cached_tail();
        cached_tail.wrapping_sub(shared_tail)
    }

    pub fn submit_wait(&self, fd: RawFd) -> io::Result<u32> {
        // Ensure that the writes into the array are not moved after the write of the tail.
        // Otherwise kernelside may read completely wrong indices from array.
        compiler_fence(Ordering::Release);
        self.array_tail.store(*self.cached_tail(), Ordering::Release);

        let retval = syscall::enter(
            fd,
            self.num_entries,
            1,
            IORING_ENTER::GETEVENTS,
            std::ptr::null(),
            0,
        )? as u32;
        // Return SQE into circulation that we successfully submitted to the kernel.
        self.increment_head(retval);
        self.notify();
        Ok(retval)
    }

    /// Submit all prepared entries to the kernel. This function will return the number of
    /// entries successfully submitted to the kernel.
    pub fn submit(&self, fd: RawFd, waker: Option<&Waker>) -> io::Result<u32> {
        if let Some(waker) = waker {
            let new = if let Some(old) = self.submitter.take() {
                if old.will_wake(waker) { old } else { waker.clone() }
            } else {
                waker.clone()
            };
            self.submitter.set(Some(new));
        }

        // Ensure that the writes into the array are not moved after the write of the tail.
        // Otherwise kernelside may read completely wrong indices from array.
        compiler_fence(Ordering::Release);
        self.array_tail.store(*self.cached_tail(), Ordering::Release);

        let retval = syscall::enter(
            fd,
            self.num_entries,
            0,
            IORING_ENTER::GETEVENTS,
            std::ptr::null(),
            0,
        )? as u32;
        // Return SQE into circulation that we successfully submitted to the kernel.
        self.increment_head(retval);
        self.notify();
        Ok(retval)
    }


    /// Prepare actions for submission by shuffling them into the correct order.
    ///
    /// Kernelside `array` is used to index into the sqes, more specifically the code behaves
    /// like this:
    /// ```C
    /// u32 mask = ctx->sq_entries - 1;
    /// u32 sq_idx = ctx->cached_sq_head++ & mask;
    /// u32 head = READ_ONCE(ctx->sq_array[sq_idx]);
    /// if (likely(head < ctx->sq_entries))
    ///     return &ctx->sq_sqes[head];
    /// ```
    /// Where `ctx->sq_entries` is the number of slots in the ring (i.e. simply a boundary check).
    ///
    /// So we need to make sure that for every new entry since we last submitted we have the
    /// correct index set. In our case shuffle will map the next `count` entries in `self.array` to
    /// point to `count` entries in `self.sqes` starting at `start`. This allows actions to be
    /// submitted to the kernel even when there are still reserved SQE in between that weren't yet
    /// filled.
    pub fn prepare(&self, start: u32, count: u32) {
        // Load the tail of the array (i.e. where we will start filling)
        let tail = self.cached_tail();
        let mut head = start;

        for _ in 0..count {
            let index = (*tail & self.ring_mask) as usize;

            // We can allow this store to be an Relaxed operation since updating the shared tail
            // is done after a memory barrier.
            self.array[index].store(head & self.ring_mask, Ordering::Relaxed);

            // Same here. We need to take the overflow into account but don't have to explicitly
            // handle it.
            head = head.wrapping_add(1);
            *tail = (*tail).wrapping_add(1);
        }

        // FIXME: This should really be done by epoll
        if let Some(waker) = self.submitter.take() {
            waker.wake_by_ref();
            self.submitter.set(Some(waker));
        }
    }

    pub fn poll_prepare<'cx>(
        self: Pin<&mut Self>,
        ctx: &mut Context<'cx>,
        count: u32,
        prepare: impl for<'sq> FnOnce(SQEs<'sq>, &mut Context<'cx>) -> Completion
    ) -> Poll<Completion> {
        if let Some(sqes) = self.try_reserve(count) {
            let start = sqes.start();
            let completion = prepare(sqes, ctx);
            self.prepare(start, count);
            Poll::Ready(completion)
        } else {
            self.waiters.push(ctx.waker().clone());
            Poll::Pending
        }
    }

    /// Suggest to submit pending events to the kernel. Returns `Ready` when the relevant event
    /// was submitted to the kernel, i.e. when kernelside `head` >= the given `head`.
    pub fn poll_submit(self: Pin<&mut Self>, ctx: &mut Context<'_>, fd: RawFd, head: u32)
        -> Poll<()>
    {
        let shared_tail = self.array_tail.load(Ordering::Relaxed);
        let cached_tail = *self.cached_tail();
        let to_submit = cached_tail.wrapping_sub(shared_tail);

        // TODO: Do some smart cookie thinking here and batch submissions in a sensible way
        if to_submit > 4 {
            self.submit(fd, None);
        }

        if *self.sqes_head() < head {
            self.waiters.push(ctx.waker().clone());
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }

    pub fn notify(&self) {
        if self.waiters.len() > 0 && self.available() > 0 {
            while let Some(waker) = self.waiters.pop() {
                waker.wake()
            }
        }
    }

    pub fn try_reserve(&self, count: u32) -> Option<SQEs<'_>> {
        if self.available() >= count {
            let start = self.increment_tail(count);
            Some(SQEs::new(self.sqes, start, count))
        } else {
            None
        }
    }
}

mod tests {
    use std::mem::ManuallyDrop;
    use std::sync::atomic::Ordering::Relaxed;
    use crate::ctypes::{IORING_OP, IOSQE};
    use super::*;

    fn gen_sq(num_entries: u32, head: u32, tail: u32) -> ManuallyDrop<SQ> {
        assert!((0 < num_entries && num_entries <= 4096), "entries must be between 1 and 4096");
        assert_eq!(num_entries.count_ones(), 1, "entries must be a power of two");

            let array_head = Box::leak(Box::new(AtomicU32::new(head)));
            let array_tail = Box::leak(Box::new(AtomicU32::new(tail)));
            let flags = Box::leak(Box::new(AtomicU32::new(0)));
            let dropped = Box::leak(Box::new(AtomicU32::new(0)));
            let array = Box::leak((0..num_entries)
                .map(|n| AtomicU32::new(n))
                .collect::<Box<[_]>>());
            let sqes = Box::leak((0..num_entries)
                .map(|_| UnsafeCell::new(io_uring_sqe::default()))
                .collect::<Box<[_]>>());

        unsafe {
            ManuallyDrop::new(SQ {
                array_head,
                sqes_head: UnsafeCell::new(head),
                array_tail,
                sqes_tail: UnsafeCell::new(tail),
                cached_tail: UnsafeCell::new(0),
                ring_mask: num_entries - 1,
                num_entries,
                flags,
                dropped,
                array,
                sqes,
                sq_ptr: NonNull::dangling(),
                sq_map_size: 0,
                sqes_map_size: 0,
                waiters: SegQueue::new(),
                submitter: Cell::new(None),
            })
        }
    }

    #[test]
    fn test_head_tail() {
        let mut sq = gen_sq(64, 30, 30);
        assert_eq!(*sq.sqes_head(), 30);
        assert_eq!(*sq.sqes_tail(), 30);
        assert_eq!(sq.used(), 0);
        assert_eq!(sq.available(), 64);

        sq.increment_tail(4);
        assert_eq!(*sq.sqes_head(), 30);
        assert_eq!(*sq.sqes_tail(), 34);
        assert_eq!(sq.used(), 4);
        assert_eq!(sq.available(), 60);

        sq.increment_head(2);
        assert_eq!(*sq.sqes_head(), 32);
        assert_eq!(*sq.sqes_tail(), 34);
        assert_eq!(sq.used(), 2);
        assert_eq!(sq.available(), 62);
    }

    #[test]
    fn test_sq_getter_setter() {
        let mut sq = gen_sq(64, 30, 30);
        assert_eq!(*sq.sqes_head(), 30);
        assert_eq!(*sq.sqes_tail(), 30);
        assert_eq!(sq.used(), 0);
        assert_eq!(sq.available(), 64);

        {
            let mut sqes = sq.try_reserve(2).unwrap();
            assert_eq!(sq.used(), 2);
            let mut sqe = sqes.next().unwrap();
            sqe.set_opcode(IORING_OP::READV);
            sqe.add_flags(IOSQE::IO_HARDLINK);
            let mut sqe = sqes.next().unwrap();
            sqe.set_opcode(IORING_OP::WRITEV);
            sqe.set_userdata(823);
        }
        assert_eq!(sq.used(), 2);

        {
            let sqes = &mut sq.sqes;
            assert_eq!(sqes[30].get_mut().opcode, IORING_OP::READV);
            assert_eq!(sqes[30].get_mut().flags, IOSQE::IO_HARDLINK);
            assert_eq!(sqes[31].get_mut().opcode, IORING_OP::WRITEV);
            assert_eq!(sqes[31].get_mut().user_data, 823);
        }


    }

    #[test]
    fn test_sq_full() {
        let mut sq = gen_sq(64, 1, 65);
        let sqe = sq.try_reserve(1);
        assert!(sqe.is_none());
    }

    #[test]
    fn test_out_of_order_submit() {
        let mut sq = gen_sq(64, 0, 0);

        let start;
        {
            let mut sqes = sq.try_reserve(4).unwrap();
            start = sqes.start();
            let mut sqe = sqes.next().unwrap();
            sqe.set_opcode(IORING_OP::READV);
            sqe.add_flags(IOSQE::IO_HARDLINK);
            sqe.set_address(1);
            let mut sqe = sqes.next().unwrap();
            sqe.set_opcode(IORING_OP::READV);
            sqe.add_flags(IOSQE::IO_HARDLINK);
            sqe.set_address(2);
            let mut sqe = sqes.next().unwrap();
            sqe.set_opcode(IORING_OP::READV);
            sqe.add_flags(IOSQE::IO_HARDLINK);
            sqe.set_address(3);
            let mut sqe = sqes.next().unwrap();
            sqe.set_opcode(IORING_OP::READV);
            sqe.set_address(4);
            sqe.set_userdata(823);
        }
        assert_eq!(sq.used(), 4);

        let start2;
        {
            let mut sqes = sq.try_reserve(4).unwrap();
            start2 = sqes.start();
            let mut sqe = sqes.next().unwrap();
            sqe.set_opcode(IORING_OP::WRITEV);
            sqe.add_flags(IOSQE::IO_LINK);
            sqe.set_address(1);
            let mut sqe = sqes.next().unwrap();
            sqe.set_opcode(IORING_OP::WRITEV);
            sqe.add_flags(IOSQE::IO_LINK);
            sqe.set_address(2);
            let mut sqe = sqes.next().unwrap();
            sqe.set_opcode(IORING_OP::WRITEV);
            sqe.add_flags(IOSQE::IO_LINK);
            sqe.set_address(3);
            let mut sqe = sqes.next().unwrap();
            sqe.set_opcode(IORING_OP::WRITEV);
            sqe.set_address(4);
            sqe.set_userdata(0xDEADBEEF);
        }
        assert_eq!(sq.used(), 8);

        sq.prepare(start2, 4);
        sq.prepare(start, 4);

        let sqes: Vec<_> = sq.sqes.iter_mut()
            .map(|c| c.get_mut().clone())
            .collect();
        let mut out: Vec<_> = sq.array.iter().map(|n| {
            let i = n.load(Relaxed) as usize;
            sqes[i]
        }).collect();

        for (n, s) in out.iter().take(4).enumerate() {
            assert_eq!(s.opcode, IORING_OP::WRITEV);
            assert_eq!(s.address, n as u64 + 1);
            if n == 3 {
                assert_eq!(s.user_data, 0xDEADBEEF);
            } else {
                assert_eq!(s.flags, IOSQE::IO_LINK);
            }
        }

        for (n, s) in out.iter().skip(4).take(4).enumerate() {
            assert_eq!(s.opcode, IORING_OP::READV);
            assert_eq!(s.address, n as u64 + 1);
            if n == 3 {
                assert_eq!(s.user_data, 823);
            } else {
                assert_eq!(s.flags, IOSQE::IO_HARDLINK);
            }
        }

        let mut i = out.iter().skip(8);
        while let Some(sqe) = i.next() {
            assert_eq!(*sqe, io_uring_sqe::default());
        }
    }
}