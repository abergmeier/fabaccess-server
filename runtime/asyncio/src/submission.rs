use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use crate::cancellation::Cancellation;
use crate::completion::Completion;
use crate::io_uring::IoUring;
use crate::sq::SQ;
use crate::sqe::{SQE, SQEs};

pub struct Submission {
    iouring: &'static IoUring,
    state: State,
}

enum State {
    Inert,
    Prepared(u32, Completion),
    Submitted(Completion),
    Cancelled(u64),
    Lost,
}

impl Submission {
    pub fn new(iouring: &'static IoUring) -> Self {
        Self { iouring, state: State::Inert }
    }

    fn split_pinned(self: Pin<&mut Self>) -> (Pin<&mut IoUring>, &mut State) {
        unsafe {
            let this = Pin::get_unchecked_mut(self);
            let iouring = &mut *(this.iouring as *const _ as *mut _);
            (Pin::new_unchecked(iouring), &mut this.state)
        }
    }

    pub fn poll(
        mut self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        count: u32,
        prepare: impl for<'sq> FnOnce(&mut SQEs<'sq>) -> SQE<'sq>
    ) -> Poll<io::Result<i32>> {
        match self.state {
            State::Inert | State::Cancelled(_) => {
                let head = crate::ready!(self.as_mut().poll_prepare(ctx, count, prepare));
                crate::ready!(self.as_mut().poll_submit(ctx, head));
                self.poll_complete(ctx)
            },
            State::Prepared(head, _) => {
                crate::ready!(self.as_mut().poll_submit(ctx, head));
                self.poll_complete(ctx)
            },
            State::Submitted(_) => self.poll_complete(ctx),
            State::Lost => {
                panic!("Ring in invalid state")
            },
        }
    }

    pub fn poll_prepare(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        count: u32,
        prepare: impl for<'sq> FnOnce(&mut SQEs<'sq>) -> SQE<'sq>
    ) -> Poll<u32> {
        let (sq, state) = self.split_pinned();
        let mut head = 0u32;
        let completion = match *state {
            State::Inert => {
                crate::ready!(sq.poll_prepare(ctx, count, |mut sqes, ctx| {
                    *state = State::Lost;

                    let mut sqe = prepare(&mut sqes);
                    let completion = Completion::new(ctx.waker().clone());
                    sqe.set_userdata(completion.addr());

                    head = sqes.used();
                    completion
                }))
            },
            State::Cancelled(prev) => {
                crate::ready!(sq.poll_prepare(ctx, count + 1, |mut sqes, ctx| {
                    *state = State::Lost;

                    sqes.soft_linked().next().unwrap().prepare_cancel(prev);

                    let mut sqe = prepare(&mut sqes);
                    let completion = Completion::new(ctx.waker().clone());
                    sqe.set_userdata(completion.addr());

                    head = sqes.used();
                    completion
                }))
            },
            _ => unreachable!(),
        };
        *state = State::Prepared(head, completion);
        Poll::Ready(head)
    }

    pub fn poll_submit(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        head: u32,
    ) -> Poll<()> {
        let (iouring, state) = self.split_pinned();
        match iouring.poll_submit(ctx, head) {
            Poll::Ready(()) => {
                match std::mem::replace(state, State::Lost) {
                    State::Prepared(_, completion) => {
                        *state = State::Submitted(completion);
                    },
                    _ => unreachable!(),
                }
                Poll::Ready(())
            },
            Poll::Pending => Poll::Pending,
        }
    }

    pub fn poll_complete(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
    ) -> Poll<io::Result<i32>> {
        let (_, state) = self.split_pinned();
        if let State::Submitted(completion) = std::mem::replace(state, State::Inert) {
            match completion.check(ctx.waker()) {
                Ok(result) => return Poll::Ready(result),
                Err(completion) => {
                    *state = State::Submitted(completion)
                }
            }
        }

        Poll::Pending
    }
}