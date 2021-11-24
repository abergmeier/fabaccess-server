use std::fs::File;
use std::future::Future;
use std::io;
use std::os::unix::prelude::AsRawFd;
use std::sync::Arc;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use asyncio::ctypes::IORING_OP;
use asyncio::io_uring::IoUring;

use futures_lite::io::AsyncReadExt;

pub fn drive<T>(iouring: &IoUring, mut f: impl Future<Output = io::Result<T>>) -> io::Result<T> {
    static VTABLE: RawWakerVTable = RawWakerVTable::new(
        |clone_me| unsafe {
            let arc = Arc::from_raw(clone_me);
            std::mem::forget(arc.clone());
            RawWaker::new(Arc::into_raw(arc) as *const (), &VTABLE)
        },
        |wake_me| unsafe { Arc::from_raw(wake_me); },
        |wake_by_ref_me| unsafe {},
        |drop_me| unsafe { drop(Arc::from_raw(drop_me)) },
    );

    let mut f = unsafe { std::pin::Pin::new_unchecked(&mut f) };
    let park = Arc::new(());
    let sender = Arc::into_raw(park.clone());
    let raw_waker = RawWaker::new(sender as *const _, &VTABLE);
    let waker = unsafe { Waker::from_raw(raw_waker) };
    let mut cx = Context::from_waker(&waker);

    loop {
        match f.as_mut().poll(&mut cx) {
            Poll::Ready(t) => return t,
            Poll::Pending => {
                iouring.handle_completions();
                match iouring.submit_wait() {
                    Ok(_) => {}
                    Err(e) => return Err(e),
                }
            }
        }
    }
}

fn main() {
    let file = File::open("/tmp/poem").unwrap();
    let fd = file.as_raw_fd();

    let mut ring: &'static IoUring = Box::leak(Box::new(IoUring::setup(4).unwrap()));

    let mut async_file = asyncio::fs::File::new(fd, ring);

    let mut buf = Box::new([0u8; 4096]);

    let f = async move {
        let len = async_file.read(&mut buf[..]).await?;
        println!("Read {} bytes:", len);
        let str = unsafe { std::str::from_utf8_unchecked(&buf[..len]) };
        println!("{}", str);
        Ok(())
    };
    drive(ring, f);
}