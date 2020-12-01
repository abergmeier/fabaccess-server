use std::pin::Pin;
use std::task::{Poll, Context};
use std::sync::Arc;
use std::future::Future;

use smol::Executor;

use futures::{future::BoxFuture, Stream, StreamExt};
use futures_signals::signal::Signal;

use crate::db::machine::MachineState;
use crate::registries::Actuator;
use crate::config::Settings;
use crate::error::Result;

pub struct Actor {
    inner: Box<dyn Actuator + Unpin>,
    f: Option<BoxFuture<'static, ()>>
}

unsafe impl Send for Actor {}

impl Actor {
    pub fn new(inner: Box<dyn Actuator + Unpin>) -> Self {
        Self { 
            inner: inner, 
            f: None,
        }
    }
}

impl Future for Actor {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let mut this = &mut *self;

        // If we have a future at the moment, poll it
        if let Some(mut f) = this.f.take() {
            if Future::poll(Pin::new(&mut f), cx).is_pending() {
                this.f.replace(f);
            }
        }

        match Stream::poll_next(Pin::new(&mut this.inner), cx) {
            Poll::Ready(None) => Poll::Ready(()),
            Poll::Ready(Some(f)) => {
                this.f.replace(f);
                Poll::Pending
            }
            Poll::Pending => Poll::Pending
        }
    }
}

pub fn load(config: &Settings) -> Result<Vec<Actor>> {
    unimplemented!()
}
