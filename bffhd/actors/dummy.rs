use futures_util::future;
use futures_util::future::BoxFuture;
use std::collections::HashMap;

use crate::actors::Actor;
use crate::db::ArchivedValue;
use crate::resources::state::State;

pub struct Dummy {
    name: String,
    params: HashMap<String, String>,
}

impl Dummy {
    pub fn new(name: String, params: HashMap<String, String>) -> Self {
        Self { name, params }
    }
}

impl Actor for Dummy {
    fn apply(&mut self, state: ArchivedValue<State>) -> BoxFuture<'static, ()> {
        tracing::info!(name=%self.name, params=?self.params, ?state, "dummy actor updating state");
        Box::pin(future::ready(()))
    }
}
