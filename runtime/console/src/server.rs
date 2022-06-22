use crate::Aggregator;
use async_channel::{Receiver, Sender};
use async_compat::CompatExt;
use console_api::instrument;
use console_api::instrument::instrument_server::{Instrument, InstrumentServer};
use console_api::tasks;
use futures_util::TryStreamExt;
use std::error::Error;
use std::future::Future;
use std::io::IoSlice;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::AsyncRead as TokioAsyncRead;
use tokio::io::{AsyncWrite as TokioAsyncWrite, ReadBuf};
use tonic::transport::server::Connected;
use tonic::Status;
use tracing_core::span::Id;

struct StreamWrapper<T>(T);
impl<T> Connected for StreamWrapper<T> {
    type ConnectInfo = ();

    fn connect_info(&self) -> Self::ConnectInfo {
        ()
    }
}
impl<T: TokioAsyncWrite + Unpin> TokioAsyncWrite for StreamWrapper<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        TokioAsyncWrite::poll_write(Pin::new(&mut self.0), cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        TokioAsyncWrite::poll_flush(Pin::new(&mut self.0), cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        TokioAsyncWrite::poll_shutdown(Pin::new(&mut self.0), cx)
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<Result<usize, std::io::Error>> {
        TokioAsyncWrite::poll_write_vectored(Pin::new(&mut self.0), cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        TokioAsyncWrite::is_write_vectored(&self.0)
    }
}
impl<T: TokioAsyncRead + Unpin> TokioAsyncRead for StreamWrapper<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        TokioAsyncRead::poll_read(Pin::new(&mut self.0), cx, buf)
    }
}

#[derive(Debug)]
pub struct Server {
    pub aggregator: Option<Aggregator>,
    client_buffer_size: usize,
    subscribe: Sender<Command>,
}

impl Server {
    //pub(crate) const DEFAULT_ADDR: IpAddr = IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1));
    pub(crate) const DEFAULT_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    pub(crate) const DEFAULT_PORT: u16 = 49289;

    pub(crate) fn new(
        aggregator: Aggregator,
        client_buffer_size: usize,
        subscribe: Sender<Command>,
    ) -> Self {
        Self {
            aggregator: Some(aggregator),
            client_buffer_size,
            subscribe,
        }
    }

    pub async fn serve(
        mut self, /*, incoming: I */
    ) -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
        let svc = InstrumentServer::new(self);

        tonic::transport::Server::builder()
            .add_service(svc)
            .serve(SocketAddr::new(Self::DEFAULT_ADDR, Self::DEFAULT_PORT))
            .compat()
            .await?;

        // TODO: Kill the aggregator task if the serve task has ended.

        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct Watch<T>(pub(crate) Sender<Result<T, tonic::Status>>);
impl<T: Clone> Watch<T> {
    pub fn update(&self, update: &T) -> bool {
        self.0.try_send(Ok(update.clone())).is_ok()
    }
}

#[derive(Debug)]
pub(crate) struct WatchRequest<T> {
    pub id: Id,
    pub stream_sender: async_oneshot::Sender<Receiver<Result<T, tonic::Status>>>,
    pub buffer: usize,
}

#[derive(Debug)]
pub(crate) enum Command {
    Instrument(Watch<instrument::Update>),
    WatchTaskDetail(WatchRequest<tasks::TaskDetails>),
    Pause,
    Resume,
}

#[tonic::async_trait]
impl Instrument for Server {
    type WatchUpdatesStream = async_channel::Receiver<Result<instrument::Update, Status>>;

    async fn watch_updates(
        &self,
        request: tonic::Request<instrument::InstrumentRequest>,
    ) -> Result<tonic::Response<Self::WatchUpdatesStream>, tonic::Status> {
        match request.remote_addr() {
            Some(addr) => tracing::debug!(client.addr = %addr, "starting a new watch"),
            None => tracing::debug!(client.addr = %"<unknown>", "starting a new watch"),
        }

        if !self.subscribe.is_full() {
            let (tx, rx) = async_channel::bounded(self.client_buffer_size);
            self.subscribe.send(Command::Instrument(Watch(tx))).await;
            tracing::debug!("watch started");
            Ok(tonic::Response::new(rx))
        } else {
            Err(tonic::Status::internal(
                "cannot start new watch, aggregation task is not running",
            ))
        }
    }

    type WatchTaskDetailsStream = async_channel::Receiver<Result<tasks::TaskDetails, Status>>;

    async fn watch_task_details(
        &self,
        request: tonic::Request<instrument::TaskDetailsRequest>,
    ) -> Result<tonic::Response<Self::WatchTaskDetailsStream>, tonic::Status> {
        let task_id = request
            .into_inner()
            .id
            .ok_or_else(|| tonic::Status::invalid_argument("missing task_id"))?
            .id;

        // `tracing` reserves span ID 0 for niche optimization for `Option<Id>`.
        let id = std::num::NonZeroU64::new(task_id)
            .map(Id::from_non_zero_u64)
            .ok_or_else(|| tonic::Status::invalid_argument("task_id cannot be 0"))?;

        if !self.subscribe.is_full() {
            // Check with the aggregator task to request a stream if the task exists.
            let (stream_sender, stream_recv) = async_oneshot::oneshot();
            self.subscribe
                .send(Command::WatchTaskDetail(WatchRequest {
                    id,
                    stream_sender,
                    buffer: self.client_buffer_size,
                }))
                .await;
            // If the aggregator drops the sender, the task doesn't exist.
            let rx = stream_recv.await.map_err(|_| {
                tracing::warn!(id = ?task_id, "requested task not found");
                tonic::Status::not_found("task not found")
            })?;

            tracing::debug!(id = ?task_id, "task details watch started");
            Ok(tonic::Response::new(rx))
        } else {
            Err(tonic::Status::internal(
                "cannot start new watch, aggregation task is not running",
            ))
        }
    }

    async fn pause(
        &self,
        _request: tonic::Request<instrument::PauseRequest>,
    ) -> Result<tonic::Response<instrument::PauseResponse>, tonic::Status> {
        self.subscribe.send(Command::Pause).await.map_err(|_| {
            tonic::Status::internal("cannot pause, aggregation task is not running")
        })?;
        Ok(tonic::Response::new(instrument::PauseResponse {}))
    }

    async fn resume(
        &self,
        _request: tonic::Request<instrument::ResumeRequest>,
    ) -> Result<tonic::Response<instrument::ResumeResponse>, tonic::Status> {
        self.subscribe.send(Command::Resume).await.map_err(|_| {
            tonic::Status::internal("cannot resume, aggregation task is not running")
        })?;
        Ok(tonic::Response::new(instrument::ResumeResponse {}))
    }
}
