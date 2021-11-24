// Imported here for modules
use std::future::Future;
use std::{fs, io};
use std::mem::ManuallyDrop;
use std::os::unix::prelude::{FromRawFd, RawFd};
use std::path::Path;
use std::pin::Pin;
use std::task::{Context, Poll};

use super::{Driver, Ring, Submission, events::*};

use futures_core::ready;
use futures_io::{AsyncRead, AsyncWrite, AsyncSeek, AsyncBufRead};

use iou::sqe::{Mode, OFlag};

pub struct File<D: Driver> {
    ring: Ring<D>,
    fd: RawFd,
    active: Op,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Op {
    Read,
    Write,
    Close,
    Nothing,
    Statx,
    Closed,
}


impl<D: Driver> File<D> {
    fn from_fd(fd: RawFd, driver: D) -> File<D> {
        File {
            ring: Ring::new(driver),
            fd,
            active: Op::Nothing,
        }
    }

    pub fn open<P: AsRef<Path>>(driver: D, path: P) -> impl Future<Output = io::Result<Self>> {
        let flags = OFlag::O_CLOEXEC | OFlag::O_RDONLY;
        open::Open(driver.submit(OpenAt::without_dir(
            path, flags, Mode::from_bits(0o666).unwrap()
        )))
    }

    pub fn create<P: AsRef<Path>>(driver: D, path: P) -> impl Future<Output = io::Result<Self>> {
        let flags = OFlag::O_CLOEXEC | OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_TRUNC;
        create::Create(driver.submit(OpenAt::without_dir(
            path, flags, Mode::from_bits(0o666).unwrap()
        )))
    }
}

mod open;
mod create;

impl<D: Driver> AsyncRead for File<D> {
    fn poll_read(mut self: Pin<&mut Self>, ctx: &mut Context<'_>, buf: &mut [u8])
        -> Poll<io::Result<usize>>
    {
        let mut inner = ready!(self.as_mut().poll_fill_buf(ctx))?;
        let len = io::Read::read(&mut inner, buf)?;
        self.consume(len);
        Poll::Ready(Ok(len))
    }
}

impl<D: Driver> AsyncBufRead for File<D> {
    fn poll_fill_buf(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        let fd = self.fd;
        let (ring, buf, pos, ..) = self.split_with_buf();
        buf.fill_buf(|buf| {
            let n = ready!(ring.poll(ctx, 1, |sqs| {
                let mut sqe = sqs.single().unwrap();
                unsafe {
                    sqe.prep_read(fd, buf, *pos);
                }
                sqe
            }))?;
            *pos += n as u64;
            Poll::Ready(Ok(n as u32))
        })
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        self.buf().consume(amt);
    }
}

impl<D: Driver> AsyncWrite for File<D> {
    fn poll_write(mut self: Pin<&mut Self>, ctx: &mut Context<'_>, slice: &[u8]) -> Poll<io::Result<usize>> {
        let fd = self.fd;
        let (ring, buf, pos, ..) = self.split_with_buf();
        let data = ready!(buf.fill_buf(|mut buf| {
            Poll::Ready(Ok(io::Write::write(&mut buf, slice)? as u32))
        }))?;
        let n = ready!(ring.poll(ctx, 1, |sqs| {
            let mut sqe = sqs.single().unwrap();
            unsafe {
                sqe.prep_write(fd, data, *pos);
            }
            sqe
        }))?;
        *pos += n as u64;
        buf.clear();
        Poll::Ready(Ok(n as usize))
    }

    fn poll_flush(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        ready!(self.poll_write(ctx, &[]))?;
        Poll::Ready(Ok(()))
    }

    fn poll_close(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.as_mut().guard_op(Op::Close);
        let fd = self.fd;
        ready!(self.as_mut().ring().poll(ctx, 1, |sqs| {
            let mut sqe = sqs.single().unwrap();
            unsafe {
                sqe.prep_close(fd);
            }
            sqe
        }))?;
        self.confirm_close();
        Poll::Ready(Ok(()))
    }
}

impl<D: Driver> AsyncSeek for File<D> {
    fn poll_seek(mut self: Pin<&mut Self>, ctx: &mut Context, pos: io::SeekFrom)
        -> Poll<io::Result<u64>>
    {
        let (start, offset) = match pos {
            io::SeekFrom::Start(n) => {
                *self.as_mut().pos() = n;
                return Poll::Ready(Ok(self.pos));
            }
            io::SeekFrom::Current(n) => (self.pos, n),
            io::SeekFrom::End(n)     => {
                (ready!(self.as_mut().poll_file_size(ctx))?, n)
            }
        };
        let valid_seek = if offset.is_negative() {
            match start.checked_sub(offset.abs() as u64) {
                Some(valid_seek) => valid_seek,
                None => {
                    let invalid = io::Error::from(io::ErrorKind::InvalidInput);
                    return Poll::Ready(Err(invalid));
                }
            }
        } else {
            match start.checked_add(offset as u64) {
                Some(valid_seek) => valid_seek,
                None => {
                    let overflow = io::Error::from_raw_os_error(libc::EOVERFLOW);
                    return Poll::Ready(Err(overflow));
                }
            }
        };
        *self.as_mut().pos() = valid_seek;
        Poll::Ready(Ok(self.pos))
    }
}

impl<D: Driver> From<File<D>> for fs::File {
    fn from(mut file: File<D>) -> fs::File {
        file.cancel();
        let file = ManuallyDrop::new(file);
        unsafe {
            fs::File::from_raw_fd(file.fd)
        }
    }
}

impl<D: Driver> Drop for File<D> {
    fn drop(&mut self) {
        match self.active {
            Op::Closed  => { }
            Op::Nothing => unsafe { libc::close(self.fd); },
            _           => self.cancel(),
        }
    }
}
