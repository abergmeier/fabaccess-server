use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::os::unix::prelude::RawFd;
use std::slice::IterMut;
use crate::ctypes::{IORING_OP, IOSQE, io_uring_sqe, SQEOpFlags};

#[derive(Debug)]
pub struct SQE<'iou> {
    sqe: &'iou mut io_uring_sqe,
}

impl<'iou> SQE<'iou> {
    pub fn new(sqe: &'iou mut io_uring_sqe) -> Self {
        Self { sqe }
    }

    #[inline(always)]
    pub fn add_flags(&mut self, flags: IOSQE) {
        self.sqe.flags |= flags;
    }

    #[inline(always)]
    pub fn set_opcode(&mut self, opcode: IORING_OP) {
        self.sqe.opcode = opcode;
    }

    #[inline(always)]
    pub fn set_userdata(&mut self, user_data: u64) {
        self.sqe.user_data = user_data;
    }

    #[inline(always)]
    pub fn set_address(&mut self, address: u64) {
        self.sqe.address = address;
    }

    #[inline(always)]
    pub fn set_len(&mut self, len: i32) {
        self.sqe.len = len;
    }

    #[inline(always)]
    pub fn set_fd(&mut self, fd: RawFd) {
        self.sqe.fd = fd;
    }

    #[inline(always)]
    pub fn set_offset(&mut self, offset: u64) {
        self.sqe.offset = offset;
    }

    #[inline(always)]
    pub fn set_op_flags(&mut self, op_flags: SQEOpFlags) {
        self.sqe.op_flags = op_flags;
    }

    pub fn prepare_cancel(&mut self, user_data: u64) {
        self.set_opcode(IORING_OP::ASYNC_CANCEL);
        self.set_address(user_data);
    }
}

pub struct SQEs<'iou> {
    slice: &'iou [UnsafeCell<io_uring_sqe>],
    mask: u32,
    start: u32,
    count: u32,
    capacity: u32,
}

impl<'iou> SQEs<'iou> {
    pub(crate) fn new(slice: &'iou [UnsafeCell<io_uring_sqe>], start: u32, capacity: u32)
        -> Self
    {
        let mask = (slice.len() - 1) as u32;
        Self { slice, mask, count: 0, start, capacity }
    }

    pub fn last(&mut self) -> Option<SQE<'iou>> {
        let mut last = None;
        while let Some(sqe) = self.consume() { last = Some(sqe) }
        last
    }

    /// An iterator of [`HardLinkedSQE`]s. These will be [`SQE`]s that are hard linked together.
    ///
    /// Hard linked SQEs will occur sequentially. All of them will be completed, even if one of the
    /// events resolves to an error.
    pub fn hard_linked(&mut self) -> HardLinked<'iou, '_> {
        HardLinked { sqes: self }
    }

    /// An iterator of [`SoftLinkedSQE`]s. These will be [`SQE`]s that are soft linked together.
    ///
    /// Soft linked SQEs will occur sequentially. If one the events errors, all events after it
    /// will be cancelled.
    pub fn soft_linked(&mut self) -> SoftLinked<'iou, '_> {
        SoftLinked { sqes: self }
    }

    /// Remaining [`SQE`]s that can be modified.
    pub fn remaining(&self) -> u32 {
        self.capacity - self.count
    }

    pub fn start(&self) -> u32 {
        self.start
    }

    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    pub fn used(&self) -> u32 {
        self.count
    }

    fn consume(&mut self) -> Option<SQE<'iou>> {
        if self.count >= self.capacity {
            None
        } else {
            let index = (self.start + self.count) & self.mask;
            self.count += 1;

            let sqe: &mut io_uring_sqe = unsafe {
                &mut *self.slice.get_unchecked(index as usize).get()
            };

            // Ensure that all SQE passing through here are wiped into NOPs first.
            *sqe = io_uring_sqe::default();
            sqe.opcode = IORING_OP::NOP;

            Some(SQE { sqe })
        }
    }

    /// Exhaust this iterator, thus ensuring all entries are set to NOP
    fn exhaust(&mut self) {
        while let Some(_) = self.consume() {}
    }
}

impl<'iou> Iterator for SQEs<'iou> {
    type Item = SQE<'iou>;

    fn next(&mut self) -> Option<SQE<'iou>> {
        self.consume()
    }
}

impl<'iou> Drop for SQEs<'iou> {
    fn drop(&mut self) {
        if self.count != 0 {
            // This iterator is responsible for all of its SQE and must NOP every not used one.
            self.exhaust()
        }
    }
}

/// An Iterator of [`SQE`]s which will be hard linked together.
pub struct HardLinked<'iou, 'a> {
    sqes: &'a mut SQEs<'iou>,
}

impl<'iou> HardLinked<'iou, '_> {
    pub fn terminate(self) -> Option<SQE<'iou>> {
        self.sqes.consume()
    }
}

impl<'iou> Iterator for HardLinked<'iou, '_> {
    type Item = HardLinkedSQE<'iou>;

    fn next(&mut self) -> Option<Self::Item> {
        let is_final = self.sqes.remaining() == 1;
        self.sqes.consume().map(|sqe| HardLinkedSQE { sqe, is_final })
    }
}

pub struct HardLinkedSQE<'iou> {
    sqe: SQE<'iou>,
    is_final: bool,
}

impl<'iou> Deref for HardLinkedSQE<'iou> {
    type Target = SQE<'iou>;

    fn deref(&self) -> &SQE<'iou> {
        &self.sqe
    }
}

impl<'iou> DerefMut for HardLinkedSQE<'iou> {
    fn deref_mut(&mut self) -> &mut SQE<'iou> {
        &mut self.sqe
    }
}

impl<'iou> Drop for HardLinkedSQE<'iou> {
    fn drop(&mut self) {
        if !self.is_final {
            self.sqe.add_flags(IOSQE::IO_HARDLINK);
        }
    }
}

/// An Iterator of [`SQE`]s which will be soft linked together.
pub struct SoftLinked<'iou, 'a> {
    sqes: &'a mut SQEs<'iou>,
}

impl<'iou> SoftLinked<'iou, '_> {
    pub fn terminate(self) -> Option<SQE<'iou>> {
        self.sqes.consume()
    }
}

impl<'iou> Iterator for SoftLinked<'iou, '_> {
    type Item = SoftLinkedSQE<'iou>;

    fn next(&mut self) -> Option<Self::Item> {
        let is_final = self.sqes.remaining() == 1;
        self.sqes.consume().map(|sqe| SoftLinkedSQE { sqe, is_final })
    }
}

pub struct SoftLinkedSQE<'iou> {
    sqe: SQE<'iou>,
    is_final: bool,
}

impl<'iou> Deref for SoftLinkedSQE<'iou> {
    type Target = SQE<'iou>;

    fn deref(&self) -> &SQE<'iou> {
        &self.sqe
    }
}

impl<'iou> DerefMut for SoftLinkedSQE<'iou> {
    fn deref_mut(&mut self) -> &mut SQE<'iou> {
        &mut self.sqe
    }
}

impl<'iou> Drop for SoftLinkedSQE<'iou> {
    fn drop(&mut self) {
        if !self.is_final {
            self.sqe.add_flags(IOSQE::IO_LINK);
        }
    }
}

mod tests {
    use super::*;

    fn gen_buf(num_entries: usize) -> &'static mut [UnsafeCell<io_uring_sqe>]{
        Box::leak((0..num_entries)
            .map(|_| UnsafeCell::new(io_uring_sqe::default()))
            .collect::<Box<[_]>>())
    }

    #[test]
    fn test_wrapping_sqes() {
        let mut sqe_buf = gen_buf(64);

        {
            let mut sqes = SQEs::new(&mut sqe_buf[..], 62, 5);
            assert_eq!(sqes.next().map(|i| i.sqe.user_data = 1), Some(()));
            assert_eq!(sqes.next().map(|i| i.sqe.user_data = 2), Some(()));
            assert_eq!(sqes.next().map(|i| i.sqe.user_data = 3), Some(()));
            assert_eq!(sqes.next().map(|i| i.sqe.user_data = 4), Some(()));
            assert_eq!(sqes.next().map(|i| i.sqe.user_data = 5), Some(()));
            assert_eq!(sqes.next().map(|i| i.sqe.user_data = 6), None);
        }

        assert_eq!(sqe_buf[61].get_mut().user_data, 0);
        assert_eq!(sqe_buf[62].get_mut().user_data, 1);
        assert_eq!(sqe_buf[63].get_mut().user_data, 2);
        assert_eq!(sqe_buf[0].get_mut().user_data, 3);
        assert_eq!(sqe_buf[1].get_mut().user_data, 4);
        assert_eq!(sqe_buf[2].get_mut().user_data, 5);
        assert_eq!(sqe_buf[3].get_mut().user_data, 0);

    }

    #[test]
    fn test_hard_linked_sqes() {
        let mut sqe_buf = gen_buf(64);

        {
            let mut sqes = SQEs::new(&mut sqe_buf, 62, 5);
            let mut linked = sqes.hard_linked();

            assert_eq!(linked.next().map(|i| i.sqe.sqe.opcode = IORING_OP::READ), Some(()));
            assert_eq!(linked.next().map(|i| i.sqe.sqe.opcode = IORING_OP::TEE), Some(()));
            assert_eq!(linked.next().map(|i| i.sqe.sqe.opcode = IORING_OP::ACCEPT), Some(()));
            assert_eq!(linked.next().map(|i| i.sqe.sqe.opcode = IORING_OP::CLOSE), Some(()));
            assert_eq!(linked.next().map(|i| i.sqe.sqe.opcode = IORING_OP::CONNECT), Some(()));
            assert_eq!(linked.next().map(|i| i.sqe.sqe.opcode = IORING_OP::FADVISE), None);
        }

        assert_eq!(sqe_buf[61].get_mut().opcode, IORING_OP::NOP);
        assert_eq!(sqe_buf[61].get_mut().flags, IOSQE::empty());

        assert_eq!(sqe_buf[62].get_mut().opcode, IORING_OP::READ);
        assert_eq!(sqe_buf[62].get_mut().flags, IOSQE::IO_HARDLINK);

        assert_eq!(sqe_buf[63].get_mut().opcode, IORING_OP::TEE);
        assert_eq!(sqe_buf[63].get_mut().flags, IOSQE::IO_HARDLINK);

        assert_eq!(sqe_buf[0].get_mut().opcode, IORING_OP::ACCEPT);
        assert_eq!(sqe_buf[0].get_mut().flags, IOSQE::IO_HARDLINK);

        assert_eq!(sqe_buf[1].get_mut().opcode, IORING_OP::CLOSE);
        assert_eq!(sqe_buf[1].get_mut().flags, IOSQE::IO_HARDLINK);

        assert_eq!(sqe_buf[2].get_mut().opcode, IORING_OP::CONNECT);
        assert_eq!(sqe_buf[2].get_mut().flags, IOSQE::empty());

        assert_eq!(sqe_buf[3].get_mut().opcode, IORING_OP::NOP);
        assert_eq!(sqe_buf[3].get_mut().flags, IOSQE::empty());
    }

    #[test]
   fn test_soft_linked_sqes() {
        let mut sqe_buf = gen_buf(64);

        {
            let mut sqes = SQEs::new(&mut sqe_buf, 62, 5);
            let mut linked = sqes.soft_linked();

            assert_eq!(linked.next().map(|i| i.sqe.sqe.opcode = IORING_OP::READ), Some(()));
            assert_eq!(linked.next().map(|i| i.sqe.sqe.opcode = IORING_OP::TEE), Some(()));
            assert_eq!(linked.next().map(|i| i.sqe.sqe.opcode = IORING_OP::ACCEPT), Some(()));
            assert_eq!(linked.next().map(|i| i.sqe.sqe.opcode = IORING_OP::CLOSE), Some(()));
            assert_eq!(linked.next().map(|i| i.sqe.sqe.opcode = IORING_OP::CONNECT), Some(()));
            assert_eq!(linked.next().map(|i| i.sqe.sqe.opcode = IORING_OP::FADVISE), None);
        }

        assert_eq!(sqe_buf[61].get_mut().opcode, IORING_OP::NOP);
        assert_eq!(sqe_buf[61].get_mut().flags, IOSQE::empty());

        assert_eq!(sqe_buf[62].get_mut().opcode, IORING_OP::READ);
        assert_eq!(sqe_buf[62].get_mut().flags, IOSQE::IO_LINK);

        assert_eq!(sqe_buf[63].get_mut().opcode, IORING_OP::TEE);
        assert_eq!(sqe_buf[63].get_mut().flags, IOSQE::IO_LINK);

        assert_eq!(sqe_buf[0].get_mut().opcode, IORING_OP::ACCEPT);
        assert_eq!(sqe_buf[0].get_mut().flags, IOSQE::IO_LINK);

        assert_eq!(sqe_buf[1].get_mut().opcode, IORING_OP::CLOSE);
        assert_eq!(sqe_buf[1].get_mut().flags, IOSQE::IO_LINK);

        assert_eq!(sqe_buf[2].get_mut().opcode, IORING_OP::CONNECT);
        assert_eq!(sqe_buf[2].get_mut().flags, IOSQE::empty());

        assert_eq!(sqe_buf[3].get_mut().opcode, IORING_OP::NOP);
        assert_eq!(sqe_buf[3].get_mut().flags, IOSQE::empty());
    }
}