use capnp::capability::Promise;
use capnp::Error;
use capnp_rpc::pry;
use rsasl::property::AuthId;
use rsasl::session::{Session, Step, StepResult};
use std::io::Cursor;

use crate::authorization::AuthorizationHandle;
use crate::capnp::session::APISession;
use crate::session::SessionManager;
use api::authenticationsystem_capnp::authentication::{
    AbortParams, AbortResults, Server as AuthenticationSystem, StepParams, StepResults,
};
use api::authenticationsystem_capnp::{response, response::Error as ErrorCode};

pub struct Authentication {
    state: State,
}

impl Authentication {
    pub fn new(session: Session, sessionmanager: SessionManager) -> Self {
        Self {
            state: State::Running(session, sessionmanager),
        }
    }

    pub fn invalid_mechanism() -> Self {
        Self {
            state: State::InvalidMechanism,
        }
    }

    fn build_error(&self, response: response::Builder) {
        if let State::Running(_, _) = self.state {
            return;
        }

        let mut builder = response.init_failed();
        match self.state {
            State::InvalidMechanism => builder.set_code(ErrorCode::BadMechanism),
            State::Finished => builder.set_code(ErrorCode::Aborted),
            State::Aborted => builder.set_code(ErrorCode::Aborted),
            _ => unreachable!(),
        }
    }
}

enum State {
    InvalidMechanism,
    Finished,
    Aborted,
    Running(Session, SessionManager),
}

impl AuthenticationSystem for Authentication {
    fn step(&mut self, params: StepParams, mut results: StepResults) -> Promise<(), Error> {
        let mut builder = results.get();
        if let State::Running(mut session, manager) =
            std::mem::replace(&mut self.state, State::Aborted)
        {
            let data: &[u8] = pry!(pry!(params.get()).get_data());
            let mut out = Cursor::new(Vec::new());
            match session.step(Some(data), &mut out) {
                Ok(Step::Done(data)) => {
                    self.state = State::Finished;

                    let uid = pry!(session.get_property::<AuthId>().ok_or(capnp::Error::failed(
                        "Authentication didn't provide an authid as required".to_string()
                    )));
                    let session = pry!(manager.open(uid.as_ref()).ok_or(capnp::Error::failed(
                        "Failed to lookup the given user".to_string()
                    )));

                    let mut builder = builder.init_successful();
                    if data.is_some() {
                        builder.set_additional_data(out.into_inner().as_slice());
                    }

                    APISession::build(session, builder)
                }
                Ok(Step::NeedsMore(_)) => {
                    self.state = State::Running(session, manager);
                    builder.set_challenge(out.into_inner().as_slice());
                }
                Err(_) => {
                    self.state = State::Aborted;
                    self.build_error(builder);
                }
            }
        } else {
            self.build_error(builder);
        }

        Promise::ok(())
    }

    fn abort(&mut self, _: AbortParams, _: AbortResults) -> Promise<(), Error> {
        self.state = State::Aborted;
        Promise::ok(())
    }
}
