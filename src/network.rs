use futures_signals::signal::Signal;

struct Network {

}

impl Network {
    pub fn new() -> Self {
        Self {  }
    }

    /// react to a signal coming in by running a future with $parameter
    // TODO: Actually take a parameter.
    pub fn react<S: Signal, F: Fn() -> ()>(&mut self, s: S, f: F) {
        unimplemented!()
    }

    /// Filter an incoming signal
    ///
    /// Idea being that bffh builds an event network that filters an incoming event into an
    /// the appropiate (sub)set of signal handlers based on pretty dynamic configuration.
    pub fn filter<B, S: Signal<Item=B>, F: Fn(&B) -> bool>(&mut self, ) {
        unimplemented!()
    }
}

enum Event {

}

trait Filter<S> {
    fn filter(&self, f: Fn(&S) -> bool);
}
