use std::pin::Pin;
use futures::task::{Context, Poll};
use futures::{Future, Stream};
use futures::future::BoxFuture;
use futures_signals::signal::Signal;
use crate::db::user::UserId;

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

pub type SensBox = Box<dyn Signal<Item=UserId> + Send + Sync>;
type Inner = HashMap<String, SensBox>;


