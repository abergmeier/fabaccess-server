use std::{
    sync::Arc,
    collections::HashMap,
};

use futures_signals::signal::{
    Mutable,
    MutableSignalCloned
};

use crate::state::State;

type ResourceState = Mutable<Arc<State>>;
type ResourceStateSignal = MutableSignalCloned<Arc<State>>;

/// Connection Broker between Resources and Subscribers
///
/// This serves as touch-off point between resources and anybody else. It doesn't drive
/// any state updates, it only allows subscribers to subscribe to the resources that are
/// driving the state updates
pub struct Network {
    sources: HashMap<u64, ResourceState>,
}


