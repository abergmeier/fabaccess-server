use sdk::initiators::{Initiator, InitiatorError, ResourceID, UpdateSink};
use sdk::BoxFuture;

#[sdk::module]
struct Dummy {
    a: u32,
    b: u32,
    c: u32,
    d: u32,
}

impl Initiator for Dummy {
    fn start_for(
        &mut self,
        machine: ResourceID,
    ) -> BoxFuture<'static, Result<(), Box<dyn InitiatorError>>> {
        todo!()
    }

    fn run(
        &mut self,
        request: &mut UpdateSink,
    ) -> BoxFuture<'static, Result<(), Box<dyn InitiatorError>>> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
