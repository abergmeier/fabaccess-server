use std::cell::UnsafeCell;
use std::os::unix::prelude::RawFd;
use std::pin::Pin;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU32, compiler_fence, Ordering};
use std::task::{Context, Poll, Waker};
use crossbeam_queue::SegQueue;
use nix::sys::mman::munmap;
use crate::completion::Completion;
use crate::cqe::CQE;
use crate::ctypes::{CQOffsets, IORING_CQ};

#[derive(Debug)]
pub struct CQ {
    /// Head of the completion queue. Moved by the program to indicate that it has consumed
    /// completions.
    ///
    /// While it's important that the kernel sees the same value as the userspace program the
    /// main problem that can happen otherwise is that the kernel assumes it lost completions
    /// which we already successfully pulled from the queue.
    head: &'static AtomicU32,

    /// Tail of the completion queue. Moved by the kernel when new completions are stored.
    ///
    /// Since this is modified by the kernel we should use atomic operations to read it, making
    /// sure both the kernel and any program have a consistent view of its contents.
    tail: &'static AtomicU32,

    /// A cached version of `tail` which additionally counts reserved slots for future
    /// completions, i.e. slots that the kernel will fill in the future.
    predicted_tail: UnsafeCell<u32>,

    ring_mask: u32,
    num_entries: u32,
    flags: &'static AtomicU32,
    entries: &'static [CQE],

    waiters: SegQueue<Waker>,

    // cq_ptr is set to `None` if we used a single mmap for both SQ and CQ.
    cq_ptr: *mut libc::c_void,
    cq_map_size: usize,
}

impl Drop for CQ {
    fn drop(&mut self) {
        if !self.cq_ptr.is_null() {
            unsafe { munmap(self.cq_ptr, self.cq_map_size) };
        }
    }
}

impl CQ {
    pub unsafe fn new(ptr: *mut libc::c_void,
                      offs: CQOffsets,
                      cq_entries: u32,
                      split_mmap: bool,
                      cq_map_size: usize,
    ) -> Self {
        // Sanity check the pointer and offsets. If these fail we were probably passed an
        // offsets from an uninitialized parameter struct.
        assert!(!ptr.is_null());
        assert_ne!(offs.head, offs.tail);

        // Eagerly extract static values. Since they won't ever change again there's no reason to
        // not read them now.
        let ring_mask = *(ptr.offset(offs.ring_mask as isize).cast());
        let num_entries = *(ptr.offset(offs.ring_entries as isize).cast());

        let head: &AtomicU32 = &*(ptr.offset(offs.head as isize).cast());
        let tail: &AtomicU32 = &*(ptr.offset(offs.tail as isize).cast());
        let predicted_tail = UnsafeCell::new(head.load(Ordering::Acquire));
        let flags: &AtomicU32 = &*(ptr.offset(offs.flags as isize).cast());
        let entries = std::slice::from_raw_parts(
            ptr.offset(offs.cqes as isize).cast(),
            cq_entries as usize
        );

        Self {
            head,
            predicted_tail,
            tail,
            ring_mask,
            num_entries,
            flags,

            entries,

            waiters: SegQueue::new(),

            // Only store a pointer if we used a separate mmap() syscall for the CQ
            cq_ptr: if split_mmap { ptr } else { std::ptr::null_mut() },
            cq_map_size,
        }
    }

    #[inline(always)]
    fn predicted_tail(&self) -> &mut u32 {
        unsafe { &mut *self.predicted_tail.get() }
    }

    #[inline(always)]
    /// Currently used + reserved slots
    pub fn used(&self) -> u32 {
        let tail = *self.predicted_tail();
        let head = self.head.load(Ordering::Relaxed);
        compiler_fence(Ordering::Acquire);
        tail.wrapping_sub(head)
    }

    #[inline(always)]
    /// Amount of available slots taking reservations into account.
    pub fn available(&self) -> u32 {
        self.num_entries - self.used()
    }

    /// Try to reserve a number of CQ slots to make sure that
    pub fn try_reserve(&self, count: u32) -> bool {
        if self.available() >= count {
            let tail = self.predicted_tail();
            *tail = (*tail).wrapping_add(count);
            true
        } else {
            false
        }
    }

    pub fn poll_reserve(self: Pin<&mut Self>, ctx: &mut Context<'_>, count: u32) -> Poll<()> {
        if self.available() >= count {
            Poll::Ready(())
        } else {
            self.waiters.push(ctx.waker().clone());
            Poll::Pending
        }
    }

    pub fn get_next(&self) -> Option<&CQE> {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Relaxed);
        if tail == head {
            None
        } else {
            compiler_fence(Ordering::Acquire);
            self.head.fetch_add(1, Ordering::Release);
            let index = (head & self.ring_mask) as usize;
            Some(&self.entries[index])
        }
    }

    pub fn ready(&self) -> u32 {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Relaxed);
        compiler_fence(Ordering::Acquire);
        tail.wrapping_sub(head)
    }

    pub fn handle(&self, handler: impl Fn(&CQE)) {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Relaxed);

        for i in head..tail {
            let index = (i & self.ring_mask) as usize;
            let cqe = &self.entries[index];
            handler(cqe);
        }

        compiler_fence(Ordering::Acquire);
        self.head.store(tail, Ordering::Release);
    }

    #[cfg(test)]
    fn test_insert_cqe(&self, cqe: impl Iterator<Item=CQE>) {
        let head = self.head.load(Ordering::Relaxed);
        let mut tail = self.tail.load(Ordering::Acquire);
        unsafe {
            for entry in cqe {
                let index = (tail & self.ring_mask) as usize;
                // Yes, this is absolutely not safe or defined behaviour in the first place. This
                // function must *never* be used outside simple testing setups.
                let ptr = &self.entries[index] as *const _ as *mut CQE;
                ptr.write(entry);
                tail += 1;

                // If we would overflow, crash instead
                assert!((tail - head) <= self.num_entries, "test_insert_cqe overflowed the buffer");
            }
        }
        self.tail.store(tail, Ordering::Release);
    }
}

mod tests {
    use std::sync::atomic::AtomicU64;
    use super::*;

    fn gen_cq(num_entries: u32) -> CQ {
        let head = Box::leak(Box::new(AtomicU32::new(0)));
        let tail = Box::leak(Box::new(AtomicU32::new(0)));
        let flags = Box::leak(Box::new(AtomicU32::new(0)));
        let entries = Box::leak((0..num_entries).map(|_| CQE::default()).collect());

        CQ {
            head,
            tail,
            predicted_tail: UnsafeCell::new(0),
            ring_mask: num_entries - 1,
            num_entries,
            flags,
            entries,
            cq_ptr: std::ptr::null_mut(),
            cq_map_size: 0,
            waiters: SegQueue::new(),
        }
    }

    #[test]
    fn test_test_insert_cqe() {
        let cq = gen_cq(4);
        cq.test_insert_cqe([
            CQE {
                user_data: 1,
                .. Default::default()
            },
            CQE {
                user_data: 2,
                .. Default::default()
            },
            CQE {
                user_data: 3,
                .. Default::default()
            },
            CQE {
                user_data: 4,
                .. Default::default()
            },
        ].into_iter());
        println!("{:?}", cq.entries);
        for i in 0..4 {
            assert_eq!(cq.entries[i].user_data, (i+1) as u64);
        }
    }

    #[test]
    #[should_panic]
    fn test_test_insert_cqe_overflow() {
        let cq = gen_cq(2);
        cq.test_insert_cqe([
            CQE {
                user_data: 1,
                .. Default::default()
            },
            CQE {
                user_data: 2,
                .. Default::default()
            },
            CQE {
                user_data: 3,
                .. Default::default()
            },
            CQE {
                user_data: 4,
                .. Default::default()
            },
        ].into_iter());
        println!("{:?}", cq.entries);
    }

    #[test]
    fn test_cq_reserve_insert() {
        let cq = gen_cq(4);
        assert_eq!(cq.tail.load(Ordering::Relaxed), 0);
        assert_eq!(cq.head.load(Ordering::Relaxed), 0);
        assert_eq!(*cq.predicted_tail(), 0);

        cq.try_reserve(2);
        assert_eq!(cq.tail.load(Ordering::Relaxed), 0);
        assert_eq!(*cq.predicted_tail(), 2);

        cq.test_insert_cqe([
            CQE {
                user_data: 1,
                .. Default::default()
            },
            CQE {
                user_data: 2,
                .. Default::default()
            },
        ].into_iter());

        assert_eq!(cq.head.load(Ordering::Relaxed), 0);
        assert_eq!(cq.tail.load(Ordering::Relaxed), 2);
        assert_eq!(*cq.predicted_tail(), 2);

        let mut o = AtomicU64::new(1);
        cq.handle(|cqe| {
            assert_eq!(cqe.user_data, o.fetch_add(1, Ordering::Relaxed))
        });
        assert_eq!(o.load(Ordering::Relaxed), 3);
    }
}