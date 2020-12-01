use std::future::Future;
use smol::Task;

use futures_signals::signal::Signal;
use crate::machine::Machine;

use crate::error::Result;

pub struct Initiator {
    machine: Box<dyn Signal<Item=Machine> + Send>,
}

impl Initiator {
    pub fn run(self) -> impl Future<Output=()> {
        futures::future::pending()
    }
}

pub fn load(config: &crate::config::Settings) -> Result<Vec<Initiator>> {
    unimplemented!()
}
