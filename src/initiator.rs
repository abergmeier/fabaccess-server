use std::future::Future;
use smol::Task;

use crate::error::Result;

pub struct Initiator {
}

impl Initiator {
    pub fn run(self) -> impl Future<Output=()> {
        futures::future::pending()
    }
}

pub fn load(config: &crate::config::Settings) -> Result<Vec<Initiator>> {
    unimplemented!()
}
