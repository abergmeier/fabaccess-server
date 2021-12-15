use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use async_channel as channel;
use async_oneshot as oneshot;
use futures_signals::signal::Signal;
use futures_util::future::BoxFuture;
use smol::future::FutureExt;
use sdk::initiators::{Initiator, InitiatorError, UpdateError, UpdateSink, UserID, ResourceID};
use crate::resource::{Error, Update};

#[derive(Clone)]
pub struct BffhUpdateSink {
    tx: channel::Sender<(Option<UserID>, sdk::initiators::State)>,
    rx: channel::Receiver<Result<(), Error>>,
}

#[async_trait::async_trait]
impl UpdateSink for BffhUpdateSink {
    async fn send(&mut self, userid: Option<UserID>, state: sdk::initiators::State)
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

impl BffhUpdateSink {
    fn new(tx: channel::Sender<(Option<UserID>, sdk::initiators::State)>,
           rx: channel::Receiver<Result<(), Error>>)
        -> Self
    {
        Self { tx, rx }
    }
}

struct Resource;
pub struct InitiatorDriver<S, I: Initiator> {
    resource_signal: S,
    resource: Option<channel::Sender<Update>>,
    error_channel: Option<oneshot::Receiver<Error>>,

    initiator: I,
    initiator_future: Option<BoxFuture<'static, Result<(), Box<dyn InitiatorError>>>>,
    update_sink: BffhUpdateSink,
    initiator_req_rx: channel::Receiver<(Option<UserID>, sdk::initiators::State)>,
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
        let update_sink = BffhUpdateSink::new(initiator_req_tx, initiator_reply_rx);
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
                match errchan.poll(cx) {
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
                match init_fut.poll(cx) {
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