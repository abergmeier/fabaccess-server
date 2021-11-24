use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};
use iou::{SQE, SQEs};
use super::{Event, Submission};

pub struct Completion<'cx> {
    inner: super::Completion,
    marker: PhantomData<fn(&'cx ()) -> &'cx ()>,
}

impl<'cx> Completion<'cx> {
    pub(crate) fn new(mut sqe: SQE<'_>, _sqes: SQEs<'_>, cx: &mut Context<'cx>) -> Self {
        let inner = super::Completion::new(cx.waker().clone());

        // Make the userdata for the (final) SQE a pointer to the waker for the task blocking on
        // this IO.
        unsafe { sqe.set_user_data(inner.addr()) };

        Self { inner, marker: PhantomData }
    }

    #[inline(always)]
    pub(crate) fn into_inner(self) -> super::Completion {
        self.inner
    }
}

pub trait Driver: Clone {
    /// Poll to prepare a number of submissions for the submission queue.
    ///
    /// If the driver has space for `count` SQE available it calls `prepare` to have said `SQE`
    /// inserted. A driver can assume that prepare will use exactly `count` slots. Using this
    /// drivers can implement backpressure by returning `Poll::Pending` if less than `count`
    /// slots are available and waking the respective task up if enough slots have become available.
    fn poll_prepare<'cx>(
        self: Pin<&mut Self>,
        ctx: &mut Context<'cx>,
        count: u32,
        prepare: impl FnOnce(SQEs<'_>, &mut Context<'cx>) -> Completion<'cx>,
    ) -> Poll<Completion<'cx>>;

    /// Suggestion for the driver to submit their queue to the kernel.
    ///
    /// This will be called by tasks after they have finished preparing submissions. Drivers must
    /// eventually submit these to the kernel but aren't required to do so right away.
    fn poll_submit(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<()>;

    /// Completion hint
    ///
    /// This should return `Poll::Ready` if an completion with the given user_data may have been
    /// received since the last call to this function. It is safe to always return `Poll::Ready`,
    /// even if no actions were completed.
    fn poll_complete(self: Pin<&mut Self>, ctx: &mut Context<'_>, user_data: u64) -> Poll<()>;

    fn submit<E: Event>(self, event: E) -> Submission<Self, E>
        where Self: Sized
    {
        Submission::new(self, event)
    }
}