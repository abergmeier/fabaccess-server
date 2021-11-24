use std::future::Future;
use futures_core::ready;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use super::{Ring, Driver, Event};

pub struct Submission<D: Driver, E: Event> {
    ring: Ring<D>,
    event: Option<E>,
}

impl<D: Driver, E: Event> Submission<D, E> {
    pub fn new(driver: D, event: E) -> Self {
        Self {
            ring: Ring::new(driver),
            event: Some(event),
        }
    }

    pub fn driver(&self) -> &D {
        self.ring.driver()
    }

    fn split_pinned(self: Pin<&mut Self>) -> (Pin<&mut Ring<D>>, &mut Option<E>) {
        unsafe {
            let this = Pin::get_unchecked_mut(self);
            (Pin::new_unchecked(&mut this.ring), &mut this.event)
        }
    }
}

impl<D: Driver, E: Event> Future for Submission<D, E> {
    type Output = (E, io::Result<u32>);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let (ring, event) = self.split_pinned();

        let result = if let Some(event) = event {
            let count = E::sqes_needed();
            ready!(ring.poll(cx, count, |sqes| unsafe { event.prepare(sqes) }))
        } else {
            panic!("polled Submission after completion")
        };

        Poll::Ready((event.take().unwrap(), result))
    }
}
