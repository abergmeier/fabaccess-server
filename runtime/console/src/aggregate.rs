use crate::Event;
use crossbeam_channel::Receiver;

pub(crate) struct Aggregator {
    events: Receiver<Event>,
}

impl Aggregator {
    pub fn new(events: Receiver<Event>) -> Self {
        Self { events }
    }
}
