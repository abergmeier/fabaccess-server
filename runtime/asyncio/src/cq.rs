use std::cell::UnsafeCell;
use std::os::unix::prelude::RawFd;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU32, Ordering};
use nix::sys::mman::munmap;
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
    cached_head: UnsafeCell<u32>,

    /// Tail of the completion queue. Moved by the kernel when new completions are stored.
    ///
    /// Since this is modified by the kernel we should use atomic operations to read it, making
    /// sure both the kernel and any program have a consistent view of its contents.
    tail: &'static AtomicU32,
    ring_mask: u32,
    num_entries: u32,
    flags: &'static AtomicU32,
    entries: &'static [CQE],

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
        let cached_head = UnsafeCell::new(head.load(Ordering::Acquire));
        let tail: &AtomicU32 = &*(ptr.offset(offs.tail as isize).cast());
        let flags: &AtomicU32 = &*(ptr.offset(offs.flags as isize).cast());
        let entries = std::slice::from_raw_parts(
            ptr.offset(offs.cqes as isize).cast(),
            cq_entries as usize
        );

        Self {
            head,
            cached_head,
            tail,
            ring_mask,
            num_entries,
            flags,

            entries,

            // Only store a pointer if we used a separate mmap() syscall for the CQ
            cq_ptr: if split_mmap { ptr } else { std::ptr::null_mut() },
            cq_map_size,
        }
    }

    pub fn get_next(&self) -> Option<&CQE> {
        let tail = self.tail.load(Ordering::Acquire);
        let head = self.head.load(Ordering::Acquire);
        if tail == head {
            None
        } else {
            self.head.fetch_add(1, Ordering::Release);
            let index = (head & self.ring_mask) as usize;
            Some(&self.entries[index])
        }
    }

    pub fn ready(&self) -> u32 {
        let tail = self.tail.load(Ordering::Acquire);
        let head = self.head.load(Ordering::Acquire);
        tail.wrapping_sub(head)
    }
}