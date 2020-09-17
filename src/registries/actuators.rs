use std::sync::Arc;
use smol::lock::RwLock;

use futures::prelude::*;

use std::collections::HashMap;

#[derive(Clone)]
pub struct Actuators {
    inner: Arc<RwLock<Inner>>,
}

pub type ActBox = Box<dyn Actuator + Sync + Send>;

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


#[async_trait]
pub trait Actuator {
    // TODO: Is it smarter to pass a (reference to?) a machine instead of 'name'? Do we need to
    // pass basically arbitrary parameters to the Actuator?
    async fn power_on(&mut self, name: String);
    async fn power_off(&mut self, name: String);
}

// This is merely a proof that Actuator *can* be implemented on a finite, known type. Yay for type
// systems with halting problems.
struct Dummy;
#[async_trait]
impl Actuator for Dummy {
    async fn power_on(&mut self, _name: String) {
        return
    }
    async fn power_off(&mut self, _name: String) {
        return
    }
}
