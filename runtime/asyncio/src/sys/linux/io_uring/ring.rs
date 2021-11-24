use std::{io, mem};
use std::pin::Pin;
use std::task::{Context, Poll};
use iou::{SQE, SQEs};
use super::{driver, Driver};
use super::Completion;

use futures_core::ready;
use crate::sys::linux::io_uring::cancellation::Cancellation;

///
pub struct Ring<D: Driver> {
    state: State,
    driver: D,
}

enum State {
    Empty,
    Prepared(Completion),
    Submitted(Completion),
    Cancelled(u64),
    Lost,
}

impl<D: Driver> Ring<D> {
    pub fn new(driver: D) -> Self {
        Self {
            state: State::Empty,
            driver,
        }
    }

    pub fn driver(&self) -> &D {
        &self.driver
    }

    fn split_pinned(self: Pin<&mut Self>) -> (&mut State, Pin<&mut D>) {
        unsafe {
            let this = Pin::get_unchecked_mut(self);
            (&mut this.state, Pin::new_unchecked(&mut this.driver))
        }
    }

    pub fn poll(
        mut self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        count: u32,
        prepare: impl for<'sq> FnOnce(&mut SQEs<'sq>) -> SQE<'sq>,
    ) -> Poll<io::Result<u32>> {
        match self.state {
            State::Empty => {
                ready!(self.as_mut().poll_prepare_empty(ctx, count, prepare));
                ready!(self.as_mut().poll_submit(ctx));
                self.poll_complete(ctx)
            },
            State::Cancelled(previous) => {
                ready!(self.as_mut().poll_prepare_canceled(ctx, previous, count, prepare));
                ready!(self.as_mut().poll_submit(ctx));
                self.poll_complete(ctx)
            },
            State::Prepared(_) => match self.as_mut().poll_complete(ctx) {
                Poll::Pending => {
                    ready!(self.as_mut().poll_submit(ctx));
                    self.poll_complete(ctx)
                },
                ready @ Poll::Ready(_) => ready,
            },
            State::Submitted(_) => self.poll_complete(ctx),
            State::Lost => panic!("Lost events, ring is now in an invalid state"),
        }
    }

    fn poll_prepare_empty(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        count: u32,
        prepare: impl for<'sq> FnOnce(&mut SQEs<'sq>) -> SQE<'sq>,
    ) -> Poll<()> {
        let (state, driver) = self.split_pinned();
        let completion = ready!(driver.poll_prepare(ctx, count, |mut sqes, ctx| {
            *state = State::Lost;
            let sqe = prepare(&mut sqes);
            let completion = driver::Completion::new(sqe, sqes, ctx);
            completion
        }));
        *state = State::Prepared(completion.into_inner());
        Poll::Ready(())
    }

    fn poll_prepare_canceled(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        previous: u64,
        count: u32,
        prepare: impl for<'sq> FnOnce(&mut SQEs<'sq>) -> SQE<'sq>,
    ) -> Poll<()> {
        let (mut state, driver) = self.split_pinned();
        let completion = ready!(driver.poll_prepare(ctx, count + 1, |mut sqes, ctx| {
            *state = State::Lost;
            unsafe { sqes.hard_linked().next().unwrap().prep_cancel(previous, 0); }
            let sqe = prepare(&mut sqes);
            let completion = driver::Completion::new(sqe, sqes, ctx);
            completion
        }));
        *state = State::Prepared(completion.into_inner());
        Poll::Ready(())
    }

    fn poll_submit(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<()> {
        let (state, driver) = self.split_pinned();
        let _ = ready!(driver.poll_submit(ctx));
        if let State::Prepared(completion) | State::Submitted(completion)
            = mem::replace(state, State::Lost)
        {
            *state = State::Submitted(completion);
            Poll::Ready(())
        } else {
            unreachable!();
        }
    }

    fn poll_complete(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<u32>> {
        let (state, driver) = self.split_pinned();
        match mem::replace(state, State::Lost) {
            State::Prepared(completion) => {
                ready!(driver.poll_complete(ctx, completion.addr()));
                match completion.check(ctx.waker()) {
                    Ok(result) => {
                        *state = State::Empty;
                        Poll::Ready(result)
                    },
                    Err(completion) => {
                        *state = State::Prepared(completion);
                        Poll::Pending
                    }
                }
            },
            State::Submitted(completion) => {
                ready!(driver.poll_complete(ctx, completion.addr()));
                match completion.check(ctx.waker()) {
                    Ok(result) => {
                        *state = State::Empty;
                        Poll::Ready(result)
                    },
                    Err(completion) => {
                        *state = State::Submitted(completion);
                        Poll::Pending
                    }
                }
            },
            _ => unreachable!(),
        }
    }

    pub fn cancel_pinned(self: Pin<&mut Self>, cancellation: Cancellation) {
        self.split_pinned().0.cancel(cancellation);
    }

    pub fn cancel(&mut self, cancellation: Cancellation) {
        self.state.cancel(cancellation)
    }
}

impl State {
    fn cancel(&mut self, cancellation: Cancellation) {
        match mem::replace(self, State::Lost) {
            State::Submitted(completion) | State::Prepared(completion) => {
                *self = State::Cancelled(completion.addr());
                completion.cancel(cancellation);
            },
            state=> {
                *self = state;
            }
        }
    }
}