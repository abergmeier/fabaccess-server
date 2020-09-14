use std::sync::Arc;
use smol::lock::RwLock;

use futures::prelude::*;

use std::collections::HashMap;

#[derive(Clone)]
pub struct Actuators {
    inner: Arc<RwLock<Inner>>,
}

type ActBox = Box<dyn Actuator
            < PowerOnFut = Future<Output = ()>
            , PowerOffFut = Future<Output = ()>
            >>;

type Inner = HashMap<String, ActBox>;

impl Actuators {
    pub fn new() -> Self {
        Actuators {
            inner: Arc::new(RwLock::new(Inner::new()))
        }
    }

    pub async fn register(&self, name: String, act: ActBox) {
        let mut wlock = self.inner.write().await;
        // TODO: Log an error or something if that name was already taken
        wlock.insert(name, act);
    }
}


pub trait Actuator {
    // TODO: Is it smarter to pass a (reference to?) a machine instead of 'name'? Do we need to
    // pass basically arbitrary parameters to the Actuator?
    type PowerOnFut: Future<Output = ()>;
    fn power_on(&mut self, name: String) -> Self::PowerOnFut;

    type PowerOffFut: Future<Output = ()>;
    fn power_off(&mut self, name: String) -> Self::PowerOffFut;
}

// This is merely a proof that Actuator *can* be implemented on a finite, known type. Yay for type
// systems with halting problems.
struct Dummy;
impl Actuator for Dummy {
    type PowerOnFut = DummyPowerOnFut;
    type PowerOffFut = DummyPowerOffFut;

    fn power_on(&mut self) -> DummyPowerOnFut {
        DummyPowerOnFut
    }
    fn power_off(&mut self) -> DummyPowerOffFut {
        DummyPowerOffFut
    }
}

use std::pin::Pin;
use futures::task::{Poll, Context};

struct DummyPowerOnFut;
impl Future for DummyPowerOnFut {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        Poll::Ready(())
    }
}
struct DummyPowerOffFut;
impl Future for DummyPowerOffFut {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        Poll::Ready(())
    }
}
