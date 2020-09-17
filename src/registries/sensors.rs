use futures::{Future, Stream};

pub struct Sensors {
    
}


// Implementing Sensors.
//
// Given the coroutine/task split stays as it is - Sensor input to machine update being one,
// machine update signal to actor doing thing being another, a Sensor implementation would send a
// Stream of futures - each future being an atomic Machine update.
#[async_trait]
pub trait Sensor: Stream<Item = Box<dyn Future<Output = ()>>> {
    async fn setup() -> Self;
}
