#![allow(non_camel_case_types)]

// Generated using bindgen-0.59.1 and then cleaned up by hand

use std::fmt::{Debug, Formatter};
use std::os::unix::prelude::RawFd;
use libc::{c_ulong, c_long, c_uint, c_int};

#[repr(C)]
#[derive(Debug, PartialEq, Eq, Copy, Clone, Default)]
/// Parameters for the io_uring_setup syscall.
///
/// Except for `flags`, `sq_thread_cpu`, `sq_thread_idle` and `wq_fd` this is filled entirely by
/// the kernel.
pub struct Params {
    /// Number of entries in the submission queue
    pub sq_entries: u32,
    /// Number of entries in the completion queue
    pub cq_entries: u32,
    /// Setup Flags passed to the kernel
    pub flags: IORING_SETUP,
    /// If `!= 0` this will pin the kernel thread for submission queue polling to a given CPU
    pub sq_thread_cpu: u32,
    /// Timeout for the submission queue polling kernel thread
    pub sq_thread_idle: u32,
    /// Bitflags of features available in the current context (i.e. as that uid with that kernel)
    pub features: IORING_FEAT,
    /// file descriptor for a previous io_uring instance to share kernel async backend. To use
    /// this you also need to set [`IORING_SETUP::ATTACH_WQ`].
    pub wq_fd: u32,

    // reserved
    _resv: [u32; 3],

    /// Submission Queue offsets
    pub sq_off: SQOffsets,
    /// Completion Queue offsets
    pub cq_off: CQOffsets,
}

impl Params {
    pub fn new(flags: IORING_SETUP) -> Self
    {
        Self {
            flags,
            .. Default::default()
        }
    }
}

#[repr(C)]
#[derive(Debug, PartialEq, Eq, Copy, Clone, Default)]
/// Submission Queue offsets
///
/// These are offsets (on top of [`IORING_OFF_SQ_RING`]) into the fd returned by `io_uring_setup`
/// at which relevant parts of information are stored. io_uring assumes this file to be mmap()ed
/// into the process memory space, thus allowing communication with the kernel using this shared
/// memory.
pub struct SQOffsets {
    pub head: u32,
    pub tail: u32,
    pub ring_mask: u32,
    pub ring_entries: u32,
    pub flags: u32,
    pub dropped: u32,
    pub array: u32,
    pub resv1: u32,
    pub resv2: u64,
}

#[repr(C)]
#[derive(Debug, PartialEq, Eq, Copy, Clone, Default)]
/// Completion Queue offsets
///
/// These are offsets (on top of [`IORING_OFF_SQ_RING`]) into the fd returned by `io_uring_setup`
/// at which relevant parts of information are stored. io_uring assumes this file to be mmap()ed
/// into the process memory space, thus allowing communication with the kernel using this shared
/// memory.
pub struct CQOffsets {
    pub head: u32,
    pub tail: u32,
    pub ring_mask: u32,
    pub ring_entries: u32,
    pub overflow: u32,
    pub cqes: u32,
    pub flags: u32,
    pub resv1: u32,
    pub resv2: u64,
}

#[repr(C)]
#[derive(Debug, PartialEq, Eq, Copy, Clone, Default)]
/// Submission Queue Event
///
/// This struct describes the I/O action that the kernel should execute on the programs behalf.
/// Every SQE will generate a [`CQE`] reply on the completion queue when the action has been
/// completed (successfully or not) which will contain the same `user_data` value. Usually
/// `user_data` is set to a pointer value, e.g. to a [`std::task::Waker`] allowing a task
/// blocking on this I/O action to be woken up.
pub struct io_uring_sqe {
    /// Type of operation for this SQE
    pub opcode: IORING_OP,
    pub flags: IOSQE,
    pub ioprio: u16,
    pub fd: RawFd,
    pub offset: u64,
    pub address: u64,
    pub len: i32,
    pub op_flags: SQEOpFlags,
    pub user_data: u64,
    pub personality: pers_buf_pad,
}

#[repr(C)]
#[derive(Eq, Copy, Clone)]
pub union SQEOpFlags {
    pub rw_flags: c_int,
    pub fsync_flags: FSYNC_FLAGS,
    pub poll_events: u16,
    pub poll32_events: u32,
    pub sync_range_flags: u32,
    pub msg_flags: u32,
    pub timeout_flags: TIMEOUT_FLAGS,
    pub accept_flags: u32,
    pub cancel_flags: u32,
    pub open_flags: u32,
    pub statx_flags: u32,
    pub fadvise_advice: u32,
    pub splice_flags: u32,
    pub rename_flags: u32,
    pub unlink_flags: u32,
}
static_assertions::assert_eq_size!(u32, SQEOpFlags);

impl PartialEq for SQEOpFlags {
    fn eq(&self, other: &Self) -> bool {
        unsafe { self.rw_flags == other.rw_flags }
    }
}

impl Default for SQEOpFlags {
    fn default() -> Self {
        Self { rw_flags: 0 }
    }
}

impl Debug for SQEOpFlags {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        unsafe {
            f.debug_struct("union ioop_flags")
                .field("raw", &self.rw_flags)
                .field("fsync_flags", &self.fsync_flags)
                .field("timeout_flags", &self.timeout_flags)
                .finish()
        }
    }
}

#[repr(C)]
#[derive(Eq, Copy, Clone)]
pub union pers_buf_pad {
    pub personality: personality,
    pub __pad2: [u64; 3],
}

impl PartialEq for pers_buf_pad {
    fn eq(&self, other: &Self) -> bool {
        unsafe { self.personality == other.personality }
    }
}

impl Default for pers_buf_pad {
    fn default() -> Self {
        Self { personality: personality::default() }
    }
}

impl Debug for pers_buf_pad {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        unsafe {
            f.debug_struct("union pers_buf_pad")
             .field("personality", &self.personality)
             .finish()
        }
    }
}

#[repr(C)]
#[derive(Debug, PartialEq, Eq, Copy, Clone, Default)]
pub struct personality {
    pub buffer: buffer_selection,
    pub personality: u16,
    pub splice_fd_in: i32,
}
#[repr(C, packed)]
#[derive(Eq, Copy, Clone)]
pub union buffer_selection {
    pub buf_index: u16,
    pub buf_group: u16,
}

impl Debug for buffer_selection {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("personality_buffer_selection")
    }
}

impl PartialEq for buffer_selection {
    fn eq(&self, other: &Self) -> bool {
        unsafe { self.buf_index == other.buf_index }
    }
}

impl Default for buffer_selection {
    fn default() -> Self {
        Self { buf_index: 0 }
    }
}

bitflags::bitflags! {
    #[derive(Default)]
    #[repr(C)]
    /// Available features
    pub struct IORING_FEAT: u32 {
        /// SQ, CQ and CQE can be mapped using a single mmap(), reducing the required mmap()s
        /// from three to two.
        const SINGLE_MMAP = 1;
        const NODROP = 2;
        const SUBMIT_STABLE = 4;
        const RW_CUR_POS = 8;
        const CUR_PERSONALITY = 16;
        const FAST_POLL = 32;
        const POLL_32BITS = 64;
        const SQPOLL_NONFIXED = 128;
        const EXT_ARG = 256;
        const NATIVE_WORKERS = 512;
        const RSRC_TAGS = 1024;
    }

    #[derive(Default)]
    #[repr(C)]
    pub struct IORING_SETUP: u32 {
        const IOPOLL = 1;
        const SQPOLL = 2;
        const SQ_AFF = 4;
        const CQSIZE = 8;
        const CLAMP = 16;
        /// Attach to an existing io_uring async backend kernel-side. This allows sharing
        /// resources while setting up several independent rings
        const ATTACH_WQ = 32;
        /// Disable the io_uring async backend on creation. This allows registering
        /// resources but prevents submitting and polling
        const R_DISABLED = 64;
    }

    #[derive(Default)]
    #[repr(C)]
    pub struct IORING_SQ: u32 {
        /// The Kernel Submission Queue thread was stopped and needs to be waked up again.
        const NEED_WAKEUP = 1;
        /// The Completion Queue as overflown and completions were dropped.
        const CQ_OVERFLOW = 2;
    }

    #[derive(Default)]
    #[repr(C)]
    pub struct IORING_CQ: u32 {
        const EVENTFD_DISABLED = 1;
    }

    #[derive(Default)]
    #[repr(C)]
    pub struct IORING_ENTER: u32 {
        /// If this flag is set, then the system call will wait for the specified number of
        /// events in `min_complete` before returning. This flag can be set along with `to_submit`
        /// to both submit and complete events in a single system call.
        const GETEVENTS = 1;
        /// If the io_uring was created with [`IORING_SETUP::SQPOLL`] then this flag asks the kernel
        /// to wake up the kernel SQ thread.
        const SQ_WAKEUP = 2;
        /// When the io_uring was created with [`IORING_SETUP::SQPOLL`] it's impossible to know
        /// for an application when the kernel as consumed an SQ event. If this flag is set
        /// io_uring_enter will block until at least one SQE was consumed and can be re-used.
        const SQ_WAIT = 4;
        /// Setting this flags allows passing extra arguments to recent enough kernel versions
        /// (>= 5.11).
        /// This allows passing arguments other than a [`libc::sigset_t`] to `io_uring_enter`
        const EXT_ARG = 8;
    }

    #[derive(Default)]
    #[repr(C)]
    pub struct IOSQE: u8 {
        /// If set a passed `fd` is not a fd but instead an index into the array of file
        /// descriptors registered using [`io_uring_register`](crate::syscall::io_uring_register).
        const FIXED_FILE = 1 << 0;

        /// When this flag is specified, the SQE will not be started before previously submitted
        /// `SQEs` have completed, and new `SQEs` will not be started before this one completes.
        /// Available since 5.2.
        const IO_DRAIN = 1 << 1;


        /// When this flag is specified, it forms a link with the next [`SQE`] in the submission
        /// ring.  That next `SQE` will not be started before this one completes. This, in effect,
        /// forms a chain of `SQEs`, which can be arbitrarily long. The tail of the chain is
        /// denoted by the first `SQE` that does not have this flag set. This flag has no effect on
        /// previous `SQE` submissions, nor does it impact `SQEs` that are outside of the chain
        /// tail. This means that multiple chains can be executing in parallel, or chains and
        /// individual `SQEs`. Only members inside the chain are serialized. A chain of `SQEs` will
        /// be broken, if any request in that chain ends in error. `io_uring` considers any
        /// unexpected result an error. This means that, eg., a short read will also terminate the
        /// remainder of the chain.  If a chain of `SQE` links is broken, the remaining unstarted
        /// part of the chain will be terminated and completed with `-ECANCELED` as the error code.
        /// Available since 5.3.
        const IO_LINK = 1 << 2;

        /// Like  [`IOSQE::IO_LINK`],  but  it doesn't sever regardless of the completion result.
        /// Note that the link will still sever if we fail submitting the parent request, hard
        /// links are only resilient in the presence of completion results for requests that did
        /// submit correctly. `IOSQE::IO_HARDLINK` implies `IO_LINK`. Available since 5.5.
        const IO_HARDLINK = 1 << 3;

        /// Normal operation for io_uring is to try and issue an sqe as non-blocking first, and if
        /// that fails, execute it in an async manner. To support more efficient overlapped
        /// operation of requests that the application knows/assumes will always (or most of the
        /// time) block, the application can ask for an sqe to be issued async from the start.
        /// Available since 5.6.
        const ASYNC = 1 << 4;

        /// Used in conjunction with the [`IORING_OP::PROVIDE_BUFFERS`] command, which registers a
        /// pool of buffers to be used by commands that read or receive data. When buffers are
        /// registered for this use case, and this flag is set in the command, io_uring will grab a
        /// buffer from this pool when the request is ready to receive or read data. If successful,
        /// the resulting `CQE` will have [`IOCQE::F_BUFFER`] set in the flags part of the struct,
        /// and the upper `IORING_CQE_BUFFER_SHIFT` bits will contain the ID of the selected
        /// buffers.  This allows the application to know exactly which buffer was selected for the
        /// op‐ eration. If no buffers are available and this flag is set, then the request will
        /// fail with `-ENOBUFS` as the error code. Once a buffer has been used, it is no longer
        /// available in the kernel pool. The application must re-register the given buffer again
        /// when it is ready to recycle it (eg has completed using it). Available since 5.7.
        const BUFFER_SELECT = 1 << 5;
    }

    #[derive(Default)]
    #[repr(C)]
    pub struct FSYNC_FLAGS: u32 {
        const DATASYNC = 1;
    }

    #[derive(Default)]
    #[repr(C)]
    pub struct TIMEOUT_FLAGS: u32 {
        const ABS = 0;
        const UPDATE = 1;
        const BOOTTIME = 1 << 2;
        const REALTIME = 1 << 3;
        const LINK_UPDATE = 1 << 4;
        const CLOCK_MASK = (Self::BOOTTIME.bits | Self::REALTIME.bits);
        const UPDATE_MASK = (Self::UPDATE.bits | Self::LINK_UPDATE.bits);
    }
}
static_assertions::assert_eq_size!(u32, IORING_FEAT);
static_assertions::assert_eq_size!(u32, IORING_SETUP);
static_assertions::assert_eq_size!(u32, IORING_SQ);
static_assertions::assert_eq_size!(u32, IORING_CQ);
static_assertions::assert_eq_size!(u32, IORING_ENTER);
static_assertions::assert_eq_size!(u8, IOSQE);

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
#[repr(u8)]
#[non_exhaustive]
pub enum IORING_OP {
    NOP = 0,
    READV = 1,
    WRITEV = 2,
    FSYNC = 3,
    READ_FIXED = 4,
    WRITE_FIXED = 5,
    POLL_ADD = 6,
    POLL_REMOVE = 7,
    SYNC_FILE_RANGE = 8,
    SENDMSG = 9,
    RECVMSG = 10,
    TIMEOUT = 11,
    TIMEOUT_REMOVE = 12,
    ACCEPT = 13,
    ASYNC_CANCEL = 14,
    LINK_TIMEOUT = 15,
    CONNECT = 16,
    FALLOCATE = 17,
    OPENAT = 18,
    CLOSE = 19,
    FILES_UPDATE = 20,
    STATX = 21,
    READ = 22,
    WRITE = 23,
    FADVISE = 24,
    MADVISE = 25,
    SEND = 26,
    RECV = 27,
    OPENAT2 = 28,
    EPOLL_CTL = 29,
    SPLICE = 30,
    PROVIDE_BUFFERS = 31,
    REMOVE_BUFFERS = 32,
    TEE = 33,
    SHUTDOWN = 34,
    RENAMEAT = 35,
    UNLINKAT = 36,
    MKDIRAT = 37,
    SYMLINKAT = 38,
    LINKAT = 39,

    LAST = 40,
}
static_assertions::assert_eq_size!(u8, IORING_OP);

impl Default for IORING_OP {
    fn default() -> Self {
        IORING_OP::NOP
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
#[repr(u32)]
#[non_exhaustive]
pub enum IORING_REGISTER_OP {
    REGISTER_BUFFERS = 0,
    UNREGISTER_BUFFERS = 1,
    REGISTER_FILES = 2,
    UNREGISTER_FILES = 3,
    REGISTER_EVENTFD = 4,
    UNREGISTER_EVENTFD = 5,
    REGISTER_FILES_UPDATE = 6,
    REGISTER_EVENTFD_ASYNC = 7,
    REGISTER_PROBE = 8,
    REGISTER_PERSONALITY = 9,
    UNREGISTER_PERSONALITY = 10,
    REGISTER_RESTRICTIONS = 11,
    REGISTER_ENABLE_RINGS = 12,
    REGISTER_LAST = 13,
}
static_assertions::assert_eq_size!(u32, IORING_REGISTER_OP);

pub const IORING_OFF_SQ_RING: u32 = 0;
pub const IORING_OFF_CQ_RING: u32 = 134217728;
pub const IORING_OFF_SQES: u32 = 268435456;

mod tests {
    use super::*;

    #[test]
    fn bindgen_test_layout_io_uring_sqe__bindgen_ty_4__bindgen_ty_1__bindgen_ty_1() {
        assert_eq!(
            ::std::mem::size_of::<buffer_selection>(),
            2usize,
            concat!(
            "Size of: ",
            stringify!(io_uring_sqe__bindgen_ty_4__bindgen_ty_1__bindgen_ty_1)
            )
        );
        assert_eq!(
            ::std::mem::align_of::<buffer_selection>(),
            1usize,
            concat!(
            "Alignment of ",
            stringify!(io_uring_sqe__bindgen_ty_4__bindgen_ty_1__bindgen_ty_1)
            )
        );
        assert_eq!(
            unsafe {
                &(*(::std::ptr::null::<buffer_selection>()))
                    .buf_index as *const _ as usize
            },
            0usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_sqe__bindgen_ty_4__bindgen_ty_1__bindgen_ty_1),
            "::",
            stringify!(buf_index)
            )
        );
        assert_eq!(
            unsafe {
                &(*(::std::ptr::null::<buffer_selection>()))
                    .buf_group as *const _ as usize
            },
            0usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_sqe__bindgen_ty_4__bindgen_ty_1__bindgen_ty_1),
            "::",
            stringify!(buf_group)
            )
        );
    }

    #[test]
    fn bindgen_test_layout_io_uring_sqe__bindgen_ty_4__bindgen_ty_1() {
        assert_eq!(
            ::std::mem::size_of::<personality>(),
            8usize,
            concat!(
            "Size of: ",
            stringify!(io_uring_sqe__bindgen_ty_4__bindgen_ty_1)
            )
        );
        assert_eq!(
            ::std::mem::align_of::<personality>(),
            4usize,
            concat!(
            "Alignment of ",
            stringify!(io_uring_sqe__bindgen_ty_4__bindgen_ty_1)
            )
        );
        assert_eq!(
            unsafe {
                &(*(::std::ptr::null::<personality>())).personality
                    as *const _ as usize
            },
            2usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_sqe__bindgen_ty_4__bindgen_ty_1),
            "::",
            stringify!(personality)
            )
        );
        assert_eq!(
            unsafe {
                &(*(::std::ptr::null::<personality>())).splice_fd_in
                    as *const _ as usize
            },
            4usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_sqe__bindgen_ty_4__bindgen_ty_1),
            "::",
            stringify!(splice_fd_in)
            )
        );
    }

    #[test]
    fn bindgen_test_layout_io_uring_sqe__bindgen_ty_4() {
        assert_eq!(
            ::std::mem::size_of::<pers_buf_pad>(),
            24usize,
            concat!("Size of: ", stringify!(io_uring_sqe__bindgen_ty_4))
        );
        assert_eq!(
            ::std::mem::align_of::<pers_buf_pad>(),
            8usize,
            concat!("Alignment of ", stringify!(io_uring_sqe__bindgen_ty_4))
        );
        assert_eq!(
            unsafe {
                &(*(::std::ptr::null::<pers_buf_pad>())).__pad2 as *const _ as usize
            },
            0usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_sqe__bindgen_ty_4),
            "::",
            stringify!(__pad2)
            )
        );
    }

    #[test]
    fn bindgen_test_layout_io_uring_sqe() {
        assert_eq!(
            ::std::mem::size_of::<io_uring_sqe>(),
            64usize,
            concat!("Size of: ", stringify!(io_uring_sqe))
        );
        assert_eq!(
            ::std::mem::align_of::<io_uring_sqe>(),
            8usize,
            concat!("Alignment of ", stringify!(io_uring_sqe))
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<io_uring_sqe>())).opcode as *const _ as usize },
            0usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_sqe),
            "::",
            stringify!(opcode)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<io_uring_sqe>())).flags as *const _ as usize },
            1usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_sqe),
            "::",
            stringify!(flags)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<io_uring_sqe>())).ioprio as *const _ as usize },
            2usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_sqe),
            "::",
            stringify!(ioprio)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<io_uring_sqe>())).fd as *const _ as usize },
            4usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_sqe),
            "::",
            stringify!(fd)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<io_uring_sqe>())).len as *const _ as usize },
            24usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_sqe),
            "::",
            stringify!(len)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<io_uring_sqe>())).user_data as *const _ as usize },
            32usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_sqe),
            "::",
            stringify!(user_data)
            )
        );
    }

    #[test]
    fn bindgen_test_layout_io_uring_sqe__bindgen_ty_3() {
        assert_eq!(
            ::std::mem::size_of::<SQEOpFlags>(),
            4usize,
            concat!("Size of: ", stringify!(io_uring_sqe__bindgen_ty_3))
        );
        assert_eq!(
            ::std::mem::align_of::<SQEOpFlags>(),
            4usize,
            concat!("Alignment of ", stringify!(io_uring_sqe__bindgen_ty_3))
        );
        assert_eq!(
            unsafe {
                &(*(::std::ptr::null::<SQEOpFlags>())).rw_flags as *const _ as usize
            },
            0usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_sqe__bindgen_ty_3),
            "::",
            stringify!(rw_flags)
            )
        );
        assert_eq!(
            unsafe {
                &(*(::std::ptr::null::<SQEOpFlags>())).fsync_flags as *const _ as usize
            },
            0usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_sqe__bindgen_ty_3),
            "::",
            stringify!(fsync_flags)
            )
        );
        assert_eq!(
            unsafe {
                &(*(::std::ptr::null::<SQEOpFlags>())).poll_events as *const _ as usize
            },
            0usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_sqe__bindgen_ty_3),
            "::",
            stringify!(poll_events)
            )
        );
        assert_eq!(
            unsafe {
                &(*(::std::ptr::null::<SQEOpFlags>())).poll32_events as *const _
                    as usize
            },
            0usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_sqe__bindgen_ty_3),
            "::",
            stringify!(poll32_events)
            )
        );
        assert_eq!(
            unsafe {
                &(*(::std::ptr::null::<SQEOpFlags>())).sync_range_flags as *const _
                    as usize
            },
            0usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_sqe__bindgen_ty_3),
            "::",
            stringify!(sync_range_flags)
            )
        );
        assert_eq!(
            unsafe {
                &(*(::std::ptr::null::<SQEOpFlags>())).msg_flags as *const _ as usize
            },
            0usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_sqe__bindgen_ty_3),
            "::",
            stringify!(msg_flags)
            )
        );
        assert_eq!(
            unsafe {
                &(*(::std::ptr::null::<SQEOpFlags>())).timeout_flags as *const _
                    as usize
            },
            0usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_sqe__bindgen_ty_3),
            "::",
            stringify!(timeout_flags)
            )
        );
        assert_eq!(
            unsafe {
                &(*(::std::ptr::null::<SQEOpFlags>())).accept_flags as *const _ as usize
            },
            0usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_sqe__bindgen_ty_3),
            "::",
            stringify!(accept_flags)
            )
        );
        assert_eq!(
            unsafe {
                &(*(::std::ptr::null::<SQEOpFlags>())).cancel_flags as *const _ as usize
            },
            0usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_sqe__bindgen_ty_3),
            "::",
            stringify!(cancel_flags)
            )
        );
        assert_eq!(
            unsafe {
                &(*(::std::ptr::null::<SQEOpFlags>())).open_flags as *const _ as usize
            },
            0usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_sqe__bindgen_ty_3),
            "::",
            stringify!(open_flags)
            )
        );
        assert_eq!(
            unsafe {
                &(*(::std::ptr::null::<SQEOpFlags>())).statx_flags as *const _ as usize
            },
            0usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_sqe__bindgen_ty_3),
            "::",
            stringify!(statx_flags)
            )
        );
        assert_eq!(
            unsafe {
                &(*(::std::ptr::null::<SQEOpFlags>())).fadvise_advice as *const _
                    as usize
            },
            0usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_sqe__bindgen_ty_3),
            "::",
            stringify!(fadvise_advice)
            )
        );
        assert_eq!(
            unsafe {
                &(*(::std::ptr::null::<SQEOpFlags>())).splice_flags as *const _ as usize
            },
            0usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_sqe__bindgen_ty_3),
            "::",
            stringify!(splice_flags)
            )
        );
        assert_eq!(
            unsafe {
                &(*(::std::ptr::null::<SQEOpFlags>())).rename_flags as *const _ as usize
            },
            0usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_sqe__bindgen_ty_3),
            "::",
            stringify!(rename_flags)
            )
        );
        assert_eq!(
            unsafe {
                &(*(::std::ptr::null::<SQEOpFlags>())).unlink_flags as *const _ as usize
            },
            0usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_sqe__bindgen_ty_3),
            "::",
            stringify!(unlink_flags)
            )
        );
    }

    #[test]
    fn bindgen_test_layout_io_sqring_offsets() {
        assert_eq!(
            ::std::mem::size_of::<SQOffsets>(),
            40usize,
            concat!("Size of: ", stringify!(io_sqring_offsets))
        );
        assert_eq!(
            ::std::mem::align_of::<SQOffsets>(),
            8usize,
            concat!("Alignment of ", stringify!(io_sqring_offsets))
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<SQOffsets>())).head as *const _ as usize },
            0usize,
            concat!(
            "Offset of field: ",
            stringify!(io_sqring_offsets),
            "::",
            stringify!(head)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<SQOffsets>())).tail as *const _ as usize },
            4usize,
            concat!(
            "Offset of field: ",
            stringify!(io_sqring_offsets),
            "::",
            stringify!(tail)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<SQOffsets>())).ring_mask as *const _ as usize },
            8usize,
            concat!(
            "Offset of field: ",
            stringify!(io_sqring_offsets),
            "::",
            stringify!(ring_mask)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<SQOffsets>())).ring_entries as *const _ as usize },
            12usize,
            concat!(
            "Offset of field: ",
            stringify!(io_sqring_offsets),
            "::",
            stringify!(ring_entries)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<SQOffsets>())).flags as *const _ as usize },
            16usize,
            concat!(
            "Offset of field: ",
            stringify!(io_sqring_offsets),
            "::",
            stringify!(flags)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<SQOffsets>())).dropped as *const _ as usize },
            20usize,
            concat!(
            "Offset of field: ",
            stringify!(io_sqring_offsets),
            "::",
            stringify!(dropped)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<SQOffsets>())).array as *const _ as usize },
            24usize,
            concat!(
            "Offset of field: ",
            stringify!(io_sqring_offsets),
            "::",
            stringify!(array)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<SQOffsets>())).resv1 as *const _ as usize },
            28usize,
            concat!(
            "Offset of field: ",
            stringify!(io_sqring_offsets),
            "::",
            stringify!(resv1)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<SQOffsets>())).resv2 as *const _ as usize },
            32usize,
            concat!(
            "Offset of field: ",
            stringify!(io_sqring_offsets),
            "::",
            stringify!(resv2)
            )
        );
    }

    #[test]
    fn bindgen_test_layout_io_cqring_offsets() {
        assert_eq!(
            ::std::mem::size_of::<CQOffsets>(),
            40usize,
            concat!("Size of: ", stringify!(io_cqring_offsets))
        );
        assert_eq!(
            ::std::mem::align_of::<CQOffsets>(),
            8usize,
            concat!("Alignment of ", stringify!(io_cqring_offsets))
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<CQOffsets>())).head as *const _ as usize },
            0usize,
            concat!(
            "Offset of field: ",
            stringify!(io_cqring_offsets),
            "::",
            stringify!(head)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<CQOffsets>())).tail as *const _ as usize },
            4usize,
            concat!(
            "Offset of field: ",
            stringify!(io_cqring_offsets),
            "::",
            stringify!(tail)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<CQOffsets>())).ring_mask as *const _ as usize },
            8usize,
            concat!(
            "Offset of field: ",
            stringify!(io_cqring_offsets),
            "::",
            stringify!(ring_mask)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<CQOffsets>())).ring_entries as *const _ as usize },
            12usize,
            concat!(
            "Offset of field: ",
            stringify!(io_cqring_offsets),
            "::",
            stringify!(ring_entries)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<CQOffsets>())).overflow as *const _ as usize },
            16usize,
            concat!(
            "Offset of field: ",
            stringify!(io_cqring_offsets),
            "::",
            stringify!(overflow)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<CQOffsets>())).cqes as *const _ as usize },
            20usize,
            concat!(
            "Offset of field: ",
            stringify!(io_cqring_offsets),
            "::",
            stringify!(cqes)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<CQOffsets>())).flags as *const _ as usize },
            24usize,
            concat!(
            "Offset of field: ",
            stringify!(io_cqring_offsets),
            "::",
            stringify!(flags)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<CQOffsets>())).resv1 as *const _ as usize },
            28usize,
            concat!(
            "Offset of field: ",
            stringify!(io_cqring_offsets),
            "::",
            stringify!(resv1)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<CQOffsets>())).resv2 as *const _ as usize },
            32usize,
            concat!(
            "Offset of field: ",
            stringify!(io_cqring_offsets),
            "::",
            stringify!(resv2)
            )
        );
    }
    #[test]
    fn bindgen_test_layout_io_uring_params() {
        assert_eq!(
            ::std::mem::size_of::<Params>(),
            120usize,
            concat!("Size of: ", stringify!(io_uring_params))
        );
        assert_eq!(
            ::std::mem::align_of::<Params>(),
            8usize,
            concat!("Alignment of ", stringify!(io_uring_params))
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<Params>())).sq_entries as *const _ as usize },
            0usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_params),
            "::",
            stringify!(sq_entries)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<Params>())).cq_entries as *const _ as usize },
            4usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_params),
            "::",
            stringify!(cq_entries)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<Params>())).flags as *const _ as usize },
            8usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_params),
            "::",
            stringify!(flags)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<Params>())).sq_thread_cpu as *const _ as usize },
            12usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_params),
            "::",
            stringify!(sq_thread_cpu)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<Params>())).sq_thread_idle as *const _ as usize },
            16usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_params),
            "::",
            stringify!(sq_thread_idle)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<Params>())).features as *const _ as usize },
            20usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_params),
            "::",
            stringify!(features)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<Params>())).wq_fd as *const _ as usize },
            24usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_params),
            "::",
            stringify!(wq_fd)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<Params>()))._resv as *const _ as usize },
            28usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_params),
            "::",
            stringify!(resv)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<Params>())).sq_off as *const _ as usize },
            40usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_params),
            "::",
            stringify!(sq_off)
            )
        );
        assert_eq!(
            unsafe { &(*(::std::ptr::null::<Params>())).cq_off as *const _ as usize },
            80usize,
            concat!(
            "Offset of field: ",
            stringify!(io_uring_params),
            "::",
            stringify!(cq_off)
            )
        );
    }

}
