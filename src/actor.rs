use std::future::Future;

use futures_signals::signal::Signal;

use crate::db::machine::MachineState;
use crate::registries::Actuator;
use crate::config::Settings;
use crate::error::Result;

pub struct Actor {
    inner: Box<dyn Actuator>
}

impl Actor {
    pub fn new(inner: Box<dyn Actuator>) -> Self {
        Self { inner }
    }

    pub fn run(self, ex: Arc<Executor>) -> impl Future<Output=()> {
        inner.for_each(|fut| {
            ex.run(fut);
        })
    }
}

pub fn load(config: &Settings) -> Result<Vec<Actor>> {
    unimplemented!()
}
