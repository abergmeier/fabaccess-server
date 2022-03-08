use std::io::Cursor;
use capnp::capability::Promise;
use capnp::Error;
use capnp_rpc::pry;
use rsasl::session::{Session, Step};

use api::auth::authentication::{
    Server,
    AbortParams,
    AbortResults,
    StepParams,
    StepResults,
};
use api::auth::response::{
    Reason,
    Action,
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

impl Server for Authentication {
    fn step(&mut self, params: StepParams, mut results: StepResults) -> Promise<(), Error> {
        use State::*;
        let new = match self.state {
            InvalidMechanism => {
                let builder = results.get();
                let mut b = builder.init_error();
                b.set_reason(Reason::BadMechanism);
                b.set_action(Action::Permanent);
                None
            },
            Finished => {
                let builder = results.get();
                let mut b = builder.init_error();
                b.set_reason(Reason::Finished);
                b.set_action(Action::Permanent);
                None
            },
            Aborted => {
                let builder = results.get();
                let mut b = builder.init_error();
                b.set_reason(Reason::Aborted);
                b.set_action(Action::Permanent);
                None
            },
            Running(ref mut session) => {
                // TODO: If null what happens?
                let data: &[u8] = pry!(pry!(params.get()).get_data());

                let mut builder = results.get();
                let mut out = Cursor::new(Vec::new());
                match session.step(Some(data), &mut out) {
                    Ok(Step::Done(data)) => {
                        let mut b = builder.init_successful();
                        let mut session_builder = b.init_session();
                        let session = super::session::Session::new();
                        session.build(&mut session_builder);
                        Some(State::Finished)
                    },
                    Ok(Step::NeedsMore(data)) => {
                        //builder.set_challenge(data.deref());
                        None
                    },
                    Err(_) => {
                        let mut b = builder.init_error();
                        b.set_reason(Reason::Aborted);
                        b.set_action(Action::Permanent);
                        Some(State::Aborted)
                    }
                }
            }
        };

        if let Some(new) = new {
            std::mem::replace(&mut self.state, new);
        }

        Promise::ok(())
    }

    fn abort(&mut self, _: AbortParams, _: AbortResults) -> Promise<(), Error> {
        std::mem::replace(&mut self.state, State::Aborted);
        Promise::ok(())
    }
}