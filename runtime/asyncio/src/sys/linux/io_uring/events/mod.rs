
mod accept;
mod close;
mod connect;
mod epoll_ctl;
mod fadvise;
mod fallocate;
mod files_update;
mod fsync;
mod openat;
mod provide_buffers;
mod read;
mod readv;
mod recv;
mod send;
mod splice;
mod statx;
mod timeout;
mod write;
mod writev;

use std::mem::ManuallyDrop;
use iou::{SQE, SQEs};
use super::Cancellation;

pub use accept::Accept;
pub use close::Close;
pub use connect::Connect;
pub use epoll_ctl::EpollCtl;
pub use fadvise::Fadvise;
pub use fallocate::Fallocate;
pub use files_update::FilesUpdate;
pub use fsync::Fsync;
pub use openat::OpenAt;
pub use provide_buffers::ProvideBuffers;
pub use read::Read;
pub use readv::ReadVectored;
pub use recv::Recv;
pub use send::Send;
pub use splice::Splice;
pub use statx::Statx;
pub use timeout::Timeout;
pub use write::Write;
pub use writev::WriteVectored;

pub trait Event {
    fn sqes_needed() -> u32;

    unsafe fn prepare<'a>(&mut self, sqs: &mut SQEs<'a>) -> SQE<'a>;

    fn cancel(_: ManuallyDrop<Self>) -> Cancellation
        where Self: Sized
    {
        Cancellation::from(())
    }
}