use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use async_channel as channel;
use async_oneshot as oneshot;
use futures_signals::signal::Signal;
use futures_util::future::BoxFuture;
use crate::resource::{Error, Update};
use crate::resource::claim::{ResourceID, UserID};
use crate::resource::state::State;

pub enum UpdateError {
    /// We're not connected to anything anymore. You can't do anything about this error and the
    /// only reason why you even get it is because your future was called a last time before
    /// being shelved so best way to handle this error is to just return from your loop entirely,
    /// cleaning up any state that doesn't survive a freeze.
    Closed,

    Denied,

    Other(Box<dyn std::error::Error + Send>),
}

pub trait InitiatorError: std::error::Error + Send {
}

pub trait Initiator {
    fn start_for(&mut self, machine: ResourceID)
        -> BoxFuture<'static, Result<(), Box<dyn InitiatorError>>>;

    fn run(&mut self, request: &mut UpdateSink)
        -> BoxFuture<'static, Result<(), Box<dyn InitiatorError>>>;
}

#[derive(Clone)]
pub struct UpdateSink {
    tx: channel::Sender<(Option<UserID>, State)>,
    rx: channel::Receiver<Result<(), Error>>,
}

impl UpdateSink {
    fn new(tx: channel::Sender<(Option<UserID>, State)>,
           rx: channel::Receiver<Result<(), Error>>)
        -> Self
    {
        Self { tx, rx }
    }

    async fn send(&mut self, userid: Option<UserID>, state: State)
        -> Result<(), UpdateError>
    {
        if let Err(_e) = self.tx.send((userid, state)).await {
            return Err(UpdateError::Closed);
        }

        match self.rx.recv().await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(Error::Denied)) => Err(UpdateError::Denied),
            Ok(Err(Error::Internal(e))) => Err(UpdateError::Other(e)),
            // RecvError is send only when the channel is closed
            Err(_) => Err(UpdateError::Closed),
        }
    }
}

struct Resource;
pub struct InitiatorDriver<S, I: Initiator> {
    // TODO: make this a static reference to the resource because it's much easier and we don't
    //       need to replace resources at runtime at the moment.
    resource_signal: S,
    resource: Option<channel::Sender<Update>>,

    // TODO: Initiators should instead
    error_channel: Option<oneshot::Receiver<Error>>,

    initiator: I,
    initiator_future: Option<BoxFuture<'static, Result<(), Box<dyn InitiatorError>>>>,
    update_sink: UpdateSink,
    initiator_req_rx: channel::Receiver<(Option<UserID>, State)>,
    initiator_reply_tx: channel::Sender<Result<(), Error>>,
}

pub struct ResourceSink {
    pub id: ResourceID,
    pub state_sink: channel::Sender<Update>,
}

impl<S: Signal<Item=ResourceSink>, I: Initiator> InitiatorDriver<S, I> {
    pub fn new(resource_signal: S, initiator: I) -> Self {
        let (initiator_reply_tx, initiator_reply_rx) = channel::bounded(1);
        let (initiator_req_tx, initiator_req_rx) = async_channel::bounded(1);
        let update_sink = UpdateSink::new(initiator_req_tx, initiator_reply_rx);
        Self {
            resource: None,
            resource_signal,
            error_channel: None,

            initiator,
            initiator_future: None,
            update_sink,
            initiator_req_rx,
            initiator_reply_tx,
        }
    }
}

impl<S: Signal<Item=ResourceSink> + Unpin, I: Initiator + Unpin> Future for InitiatorDriver<S, I> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.resource_signal).poll_change(cx) {
            Poll::Ready(Some(resource)) => {
                self.resource = Some(resource.state_sink);
                self.error_channel = None;
                let f = Box::pin(self.initiator.start_for(resource.id));
                self.initiator_future.replace(f);
            },
            Poll::Ready(None) => self.resource = None,
            Poll::Pending => {}
        }

        // do while there is work to do
        while {
            // First things first:
            // If we've send an update to the resource in question we have error channel set, so
            // we poll that first to determine if the resource has acted on it yet.
            if let Some(ref mut errchan) = self.error_channel {
                match Pin::new(errchan).poll(cx) {
                    // In case there's an ongoing
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Ok(error)) => {
                        self.error_channel = None;
                        self.initiator_reply_tx.send(Err(error));
                    }
                    Poll::Ready(Err(_closed)) => {
                        // Error channel was dropped which means there was no error
                        self.error_channel = None;
                        self.initiator_reply_tx.send(Ok(()));
                    }
                }
            }

            if let Some(ref mut init_fut) = self.initiator_future {
                match init_fut.as_mut().poll(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Ok(())) => {},
                    Poll::Ready(Err(_e)) => {
                        // TODO: Log initiator error here
                    }
                }
            } else if let Some(ref mut resource) = self.resource {
                let mut s = self.update_sink.clone();
                let f = self.initiator.run(&mut s);
                self.initiator_future.replace(f);
            }

            self.error_channel.is_some()
        } {}

        Poll::Ready(())
    }
}