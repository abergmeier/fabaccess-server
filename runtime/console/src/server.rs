use crate::Aggregator;
use console_api::instrument::instrument_server::{Instrument, InstrumentServer};
use console_api::instrument::{
    InstrumentRequest, PauseRequest, PauseResponse, ResumeRequest, ResumeResponse,
    TaskDetailsRequest,
};
use std::error::Error;
use std::net::{IpAddr, Ipv6Addr};

pub struct Server {
    aggregator: Aggregator,
    client_buffer_size: usize,
}

impl Server {
    pub(crate) const DEFAULT_ADDR: IpAddr = IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1));
    pub(crate) const DEFAULT_PORT: u16 = 49289;

    pub(crate) fn new(aggregator: Aggregator, client_buffer_size: usize) -> Self {
        Self {
            aggregator,
            client_buffer_size,
        }
    }

    pub(crate) async fn serve(
        mut self, /*, incoming: I */
    ) -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
        // TODO: Spawn two tasks; the aggregator that's collecting stats, aggregating and
        //       collating them and the server task doing the tonic gRPC stuff

        let svc = InstrumentServer::new(self);

        // The gRPC server task; requires a `Stream` of `tokio::AsyncRead + tokio::AsyncWrite`.
        // TODO: Pass an async listening socket that implements the tokio versions of Read/Write
        let incoming = todo!();
        tonic::transport::Server::builder()
            .add_service(svc)
            .serve_with_incoming(incoming)
            .await?;

        // TODO: Kill the aggregator task if the serve task has ended.

        Ok(())
    }
}

#[tonic::async_trait]
impl Instrument for Server {
    type WatchUpdatesStream = ();

    async fn watch_updates(
        &self,
        request: tonic::Request<InstrumentRequest>,
    ) -> Result<tonic::Response<Self::WatchUpdatesStream>, tonic::Status> {
        /*
        match request.remote_addr() {
            Some(addr) => tracing::debug!(client.addr = %addr, "starting a new watch"),
            None => tracing::debug!(client.addr = %"<unknown>", "starting a new watch"),
        }
        let permit = self.subscribe.reserve().await.map_err(|_| {
            tonic::Status::internal("cannot start new watch, aggregation task is not running")
        })?;
        let (tx, rx) = mpsc::channel(self.client_buffer);
        permit.send(Command::Instrument(Watch(tx)));
        tracing::debug!("watch started");
        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(tonic::Response::new(stream))
         */
        todo!()
    }

    type WatchTaskDetailsStream = ();

    async fn watch_task_details(
        &self,
        request: tonic::Request<TaskDetailsRequest>,
    ) -> Result<tonic::Response<Self::WatchTaskDetailsStream>, tonic::Status> {
        todo!()
    }

    async fn pause(
        &self,
        request: tonic::Request<PauseRequest>,
    ) -> Result<tonic::Response<PauseResponse>, tonic::Status> {
        todo!()
    }

    async fn resume(
        &self,
        request: tonic::Request<ResumeRequest>,
    ) -> Result<tonic::Response<ResumeResponse>, tonic::Status> {
        todo!()
    }
}
