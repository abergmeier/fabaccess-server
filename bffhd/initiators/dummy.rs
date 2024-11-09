use miette::{miette, Diagnostic};
use thiserror::Error;

use super::Initiator;
use crate::initiators::InitiatorCallbacks;
use crate::resources::modules::fabaccess::Status;
use crate::session::SessionHandle;
use async_io::Timer;
use futures_util::future::BoxFuture;
use futures_util::ready;
use std::collections::HashMap;
use std::future::Future;
use std::mem;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

pub struct Dummy {
    callbacks: InitiatorCallbacks,
    session: SessionHandle,
    state: DummyState,
}

enum DummyState {
    Empty,
    Sleeping(Timer, Option<Status>),
    Updating(BoxFuture<'static, Status>),
}

impl Dummy {
    fn timer() -> Timer {
        Timer::after(Duration::from_secs(2))
    }

    fn flip(&self, status: Status) -> BoxFuture<'static, Status> {
        let session = self.session.clone();
        let mut callbacks = self.callbacks.clone();
        Box::pin(async move {
            let next = match &status {
                Status::Free => Status::InUse(session.get_user_ref()),
                Status::InUse(_) => Status::Free,
                _ => Status::Free,
            };
            callbacks.try_update(session, status).await;

            next
        })
    }
}

#[derive(Debug, Error, Diagnostic)]
pub enum DummyError {}

impl Future for Dummy {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let span = tracing::debug_span!("Dummy initiator poll");
        let _guard = span.enter();
        tracing::trace!("polling Dummy initiator");
        loop {
            match &mut self.state {
                DummyState::Empty => {
                    tracing::trace!("Dummy initiator is empty, initializing…");
                    mem::replace(
                        &mut self.state,
                        DummyState::Sleeping(Self::timer(), Some(Status::Free)),
                    );
                }
                DummyState::Sleeping(timer, next) => {
                    tracing::trace!("Sleep timer exists, polling it.");

                    let _: Instant = ready!(Pin::new(timer).poll(cx));

                    tracing::trace!("Timer has fired, poking out an update!");

                    let status = next.take().unwrap();
                    let f = self.flip(status);
                    mem::replace(&mut self.state, DummyState::Updating(f));
                }
                DummyState::Updating(f) => {
                    tracing::trace!("Update future exists, polling it .");

                    let next = ready!(Pin::new(f).poll(cx));

                    tracing::trace!("Update future completed, sleeping!");

                    mem::replace(
                        &mut self.state,
                        DummyState::Sleeping(Self::timer(), Some(next)),
                    );
                }
            }
        }
    }
}

impl Initiator for Dummy {
    fn new(params: &HashMap<String, String>, callbacks: InitiatorCallbacks) -> miette::Result<Self>
    where
        Self: Sized,
    {
        let uid = params
            .get("uid")
            .ok_or_else(|| miette!("Dummy initiator configured without an UID"))?;
        let session = callbacks
            .open_session(uid)
            .ok_or_else(|| miette!("The configured user for the dummy initiator does not exist"))?;

        Ok(Self {
            callbacks,
            session,
            state: DummyState::Empty,
        })
    }
}
