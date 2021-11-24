use std::io;
use std::ptr::NonNull;
use std::sync::atomic::Ordering;
use crate::cq::CQ;
use crate::io_uring::{IoUring};

#[repr(C)]
#[derive(Debug, PartialEq, Eq, Copy, Clone, Default)]
/// Completion Queue Event
pub struct CQE {
    pub user_data: u64,
    res: i32,
    pub flags: IOCQE,
}

impl CQE {
    pub fn raw_result(&self) -> i32 {
        self.res
    }

    pub fn result(&self) -> io::Result<i32> {
        if self.res < 0 {
            let err = io::Error::from_raw_os_error(-self.res);
            Err(err)
        } else {
            Ok(self.res)
        }
    }
}

pub struct CQEs<'a> {
    cq: &'a CQ,
    ready: u32,
}

impl<'a> CQEs<'a> {
    pub fn new(cq: &'a CQ) -> Self {
        Self { cq, ready: 0 }
    }

    fn get(&mut self) -> Option<CQE> {
        self.cq.get_next().map(|cqe| *cqe)
    }

    fn ready(&mut self) -> u32 {
        self.cq.ready()
    }
}

impl<'a> Iterator for CQEs<'a> {
    type Item = CQE;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ready == 0 {
            self.ready = self.ready();
            if self.ready == 0 {
                return None;
            }
        }

        self.ready -= 1;
        self.get()
    }
}

bitflags::bitflags! {
    #[derive(Default)]
    #[repr(C)]
    pub struct IOCQE: u32 {
        const F_BUFFER = 1;
        const F_MORE = 1 << 1;
    }
}
static_assertions::assert_eq_size!(u32, IOCQE);

mod tests {
    use super::*;

    #[test]
    fn test_result_into_std() {
        let cqe = CQE { res: 0, .. Default::default() };
        assert_eq!(cqe.result().unwrap(), 0);
        let cqe = CQE { res: 42567, .. Default::default() };
        assert_eq!(cqe.result().unwrap(), 42567);

        let cqe = CQE { res: -32, .. Default::default() };
        assert_eq!(cqe.result().unwrap_err().kind(), io::ErrorKind::BrokenPipe);

        let cqe = CQE { res: -2, .. Default::default() };
        assert_eq!(cqe.result().unwrap_err().kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn test_layout_io_uring_cqe() {
        assert_eq!(
            ::std::mem::size_of::<CQE>(),
            16usize,
            concat!("Size of: ", stringify!(io_uring_cqe))
        );
        assert_eq!(
            ::std::mem::align_of::<CQE>(),
            8usize,
            concat!("Alignment of ", stringify!(io_uring_cqe))
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<CQE>())).user_data as *const _ as usize },
            0usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_cqe),
            "::",
            stringify!(user_data)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<CQE>())).res as *const _ as usize },
            8usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_cqe),
            "::",
            stringify!(res)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<CQE>())).flags as *const _ as usize },
            12usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_cqe),
            "::",
            stringify!(flags)
            )
        );
    }

}