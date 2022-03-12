use std::io::Cursor;
use capnp::capability::Promise;
use capnp::Error;
use capnp_rpc::pry;
use rsasl::session::{Session, Step};

use api::authenticationsystem_capnp::authentication::{
    Server as AuthenticationSystem,
    StepParams, StepResults,
    AbortParams, AbortResults,
};

pub struct Authentication {
    state: State,
}

enum State {
    InvalidMechanism,
    Finished,
    Aborted,
    Running(Session)
}

impl AuthenticationSystem for Authentication {
    fn step(&mut self, params: StepParams, mut results: StepResults) -> Promise<(), Error> {
        unimplemented!();
    }

    fn abort(&mut self, _: AbortParams, _: AbortResults) -> Promise<(), Error> {
        self.state = State::Aborted;
        Promise::ok(())
    }
}