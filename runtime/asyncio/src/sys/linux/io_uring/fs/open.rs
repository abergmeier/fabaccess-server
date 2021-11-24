use std::future::Future;
use futures_core::ready;
use super::*;

pub(super) struct Open<D: Driver>(pub(super) Submission<D, OpenAt>);

impl<D: Driver> Future for Open<D> {
    type Output = io::Result<File<D>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut inner = unsafe {
            self.map_unchecked_mut(|this| &mut this.0)
        };
        let (_, ready) = ready!(inner.as_mut().poll(cx));
        let fd = ready? as i32;
        Poll::Ready(Ok(File::from_fd(fd, inner.driver().clone())))
    }
}