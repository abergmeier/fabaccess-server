use capnp::capability::Promise;
use capnp::Error;
use capnp_rpc::pry;
use rsasl::mechname::Mechname;
use rsasl::prelude::State as SaslState;
use rsasl::prelude::{MessageSent, Session};
use std::fmt;
use std::fmt::{Formatter, Write};
use tracing::Span;

use crate::authentication::V;
use crate::capnp::session::APISession;
use crate::session::SessionManager;
use api::authenticationsystem_capnp::authentication::{
    AbortParams, AbortResults, Server as AuthenticationSystem, StepParams, StepResults,
};
use api::authenticationsystem_capnp::{response, response::Error as ErrorCode};

const TARGET: &str = "bffh::api::authenticationsystem";

pub struct Authentication {
    span: Span,
    state: State,
}

impl Authentication {
    pub fn new(
        parent: &Span,
        mechanism: &Mechname, /* TODO: this is stored in session as well, get it out of there. */
        session: Session<V>,
        sessionmanager: SessionManager,
    ) -> Self {
        let span = tracing::info_span!(
            target: TARGET,
            parent: parent,
            "Authentication",
            mechanism = mechanism.as_str()
        );
        tracing::trace!(
            target: TARGET,
            parent: &span,
            "constructing valid authentication system"
        );
        Self {
            span,
            state: State::Running(session, sessionmanager),
        }
    }

    pub fn invalid_mechanism() -> Self {
        let span = tracing::info_span!(target: TARGET, "Authentication",);
        tracing::trace!(
            target: TARGET,
            parent: &span,
            "constructing invalid mechanism authentication system"
        );
        Self {
            span,
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

impl fmt::Display for Authentication {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("Authentication(")?;
        match &self.state {
            State::InvalidMechanism => f.write_str("invalid mechanism")?,
            State::Finished => f.write_str("finished")?,
            State::Aborted => f.write_str("aborted")?,
            State::Running(_, _) => f.write_str("running")?,
        }
        f.write_char(')')
    }
}

enum State {
    InvalidMechanism,
    Finished,
    Aborted,
    Running(Session<V>, SessionManager),
}

impl AuthenticationSystem for Authentication {
    fn step(&mut self, params: StepParams, mut results: StepResults) -> Promise<(), Error> {
        let _guard = self.span.enter();
        let _span = tracing::trace_span!(target: TARGET, "step",).entered();

        tracing::trace!(params.data = "<authentication data>", "method call");

        #[repr(transparent)]
        struct Response {
            union_field: &'static str,
        }
        impl fmt::Display for Response {
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                f.write_str("Response(")?;
                f.write_str(self.union_field)?;
                f.write_char(')')
            }
        }
        let response;

        let mut builder = results.get();
        if let State::Running(mut session, manager) =
            std::mem::replace(&mut self.state, State::Aborted)
        {
            let data: &[u8] = pry!(pry!(params.get()).get_data());

            let mut out = Vec::new();
            match session.step(Some(data), &mut out) {
                Ok(SaslState::Finished(sent)) => {
                    self.state = State::Finished;

                    if let Some(user) = session.validation() {
                        let session = manager.open(&self.span, user);
                        response = Response {
                            union_field: "successful",
                        };

                        let mut builder = builder.init_successful();
                        if sent == MessageSent::Yes {
                            builder.set_additional_data(out.as_slice());
                        }

                        APISession::build(session, builder)
                    } else {
                        let mut builder = builder.init_failed();
                        builder.set_code(ErrorCode::InvalidCredentials);

                        response = Response {
                            union_field: "error",
                        };
                    }
                }
                Ok(SaslState::Running) => {
                    self.state = State::Running(session, manager);
                    builder.set_challenge(out.as_slice());

                    response = Response {
                        union_field: "challenge",
                    };
                }
                Err(_) => {
                    self.state = State::Aborted;
                    self.build_error(builder);

                    response = Response {
                        union_field: "error",
                    };
                }
            }
        } else {
            self.build_error(builder);
            response = Response {
                union_field: "error",
            };
        }

        tracing::trace!(
            results = %response,
            "method return"
        );

        Promise::ok(())
    }

    fn abort(&mut self, _: AbortParams, _: AbortResults) -> Promise<(), Error> {
        let _guard = self.span.enter();
        let _span = tracing::trace_span!(
            target: TARGET,
            parent: &self.span,
            "abort",
        )
        .entered();

        tracing::trace!("method call");

        self.state = State::Aborted;

        tracing::trace!("method return");
        Promise::ok(())
    }
}
