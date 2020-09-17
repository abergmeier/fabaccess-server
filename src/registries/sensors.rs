use std::pin::Pin;
use futures::task::{Context, Poll};
use futures::{Future, Stream};
use futures::future::BoxFuture;

use std::sync::Arc;
use smol::lock::RwLock;
use std::collections::HashMap;

#[derive(Clone)]
pub struct Sensors {
    inner: Arc<RwLock<Inner>>,
}

impl Sensors {
    pub fn new() -> Self {
        Sensors {
            inner: Arc::new(RwLock::new(Inner::new())),
        }
    }
}

pub type SensBox = Box<dyn Sensor>;
type Inner = HashMap<String, SensBox>;


// Implementing Sensors.
//
// Given the coroutine/task split stays as it is - Sensor input to machine update being one,
// machine update signal to actor doing thing being another, a Sensor implementation would send a
// Stream of futures - each future being an atomic Machine update.
#[async_trait]
/// BFFH Sensor
///
/// A sensor is anything that can forward an intent of an user to do something to bffh.
/// This may be a card reader connected to a machine, a website allowing users to select a machine
/// they want to use or something like QRHello
pub trait Sensor: Stream<Item = BoxFuture<'static, ()>> {
    /// Setup the Sensor.
    ///
    /// After this async function completes the Stream implementation should be able to generate
    /// futures when polled.
    /// Implementations can rely on this function being polled to completeion before the stream
    /// is polled.
    // TODO Is this sensible vs just having module-specific setup fns?
    async fn setup(&mut self);

    /// Shutdown the sensor gracefully
    ///
    /// Implementations can rely on that the stream will not be polled after this function has been
    /// called.
    async fn shutdown(&mut self);
}

struct Dummy;
#[async_trait]
impl Sensor for Dummy {
    async fn setup(&mut self) {
        return;
    }

    async fn shutdown(&mut self) {
        return;
    }
}

impl Stream for Dummy {
    type Item = BoxFuture<'static, ()>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        Poll::Ready(Some(Box::pin(futures::future::ready(()))))
    }
}
