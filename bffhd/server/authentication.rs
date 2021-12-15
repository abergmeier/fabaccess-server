use api::utils::l10n_string;
use crate::error;

use std::ops::Deref;
use capnp::capability::Promise;
use capnp::Error;
use capnp_rpc::pry;

use rsasl::{rsasl_err_to_str, SASL, Session as SaslSession, Property, ReturnCode, RSASL, DiscardOnDrop, Mechanisms};
use rsasl::session::Step::{Done, NeedsMore};

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
use crate::users::{UserDB, PassDB};

#[derive(Debug)]
pub struct AuthenticationProvider {
    sasl: RSASL<AppData, SessionData>,
}

impl AuthenticationProvider {
    pub fn new() -> error::Result<Self> {
        let sasl = SASL::new()?;
        Ok(Self { sasl })
    }

    pub fn mechanisms(&self) -> error::Result<Mechanisms> {
        Ok(self.sasl.server_mech_list()?)
    }

    pub fn try_start_session(&mut self, mechanism: &str) -> error::Result<Authentication> {
        let session = self.sasl.server_start(mechanism)?;
        Ok(Authentication {
            state: State::Running(session),
        })
    }

    pub fn bad_mechanism(&self) -> Authentication {
        Authentication {
            state: State::InvalidMechanism,
        }
    }

    pub fn start_session(&mut self, mechanism: &str) -> Authentication {
        self.try_start_session(mechanism)
            .unwrap_or_else(|_| self.bad_mechanism())
    }
}

#[derive(Debug)]
struct Callback;

#[derive(Debug)]
struct AppData {
    userdb: UserDB,
    passdb: PassDB,
}

#[derive(Debug)]
struct SessionData;

impl rsasl::Callback<AppData, SessionData> for Callback {
    fn callback(sasl: &mut SASL<AppData, SessionData>,
                session: &mut SaslSession<SessionData>,
                prop: Property
        ) -> Result<(), ReturnCode>
    {
        match prop {
            Property::GSASL_VALIDATE_SIMPLE => {
                // Access the authentication id, i.e. the username to check the password for
                let authcid = session
                    .get_property(Property::GSASL_AUTHID)
                    .ok_or(rsasl::GSASL_NO_AUTHID)
                    .map_err(|_| rsasl::GSASL_NO_AUTHID)
                    .and_then(|cstr| cstr.to_str()
                        .map_err(|_| rsasl::GSASL_NO_AUTHID))?;

                // Access the password itself
                let password = session
                    .get_property(Property::GSASL_PASSWORD)
                    .ok_or(rsasl::GSASL_NO_PASSWORD)
                    .and_then(|cstr| cstr.to_str()
                        .map_err(|_| rsasl::GSASL_NO_AUTHID))?;

                let AppData { userdb: _, passdb } = sasl.retrieve_mut()
                    .ok_or(rsasl::GSASL_NO_CALLBACK)?;

                if let Ok(Some(Ok(true))) = passdb.verify_password(authcid, &password.as_bytes()) {
                    Ok(())
                } else {
                    Err(rsasl::GSASL_AUTHENTICATION_ERROR)
                }
            },
            _ => Err(rsasl::GSASL_NO_CALLBACK),
        }
    }
}

#[derive(Debug)]
pub struct Authentication {
    state: State<SessionData>,
}

#[derive(Debug)]
enum State<E> {
    InvalidMechanism,
    Finished,
    Aborted,
    Running(DiscardOnDrop<SaslSession<E>>)
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
                match session.step(data) {
                    Ok(Done(data)) => {
                        let mut b = builder.init_successful();
                        if !data.is_empty() {
                            b.reborrow().set_additional_data(data.deref())
                        }
                        let mut session_builder = b.init_session();
                        let session = super::session::Session::new();
                        session.build(&mut session_builder);
                        Some(State::Finished)
                    },
                    Ok(NeedsMore(data)) => {
                        builder.set_challenge(data.deref());
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

#[repr(transparent)]
struct SaslE {
    e: ReturnCode,
}

impl l10n_string::Server for SaslE {
    fn get(&mut self,
           params: l10n_string::GetParams,
           mut results: l10n_string::GetResults
    ) -> Promise<(), Error>
    {
        let lang = pry!(pry!(params.get()).get_lang());
        if lang == "en" {
            let mut builder = results.get();
            builder.set_lang("en");
            builder.set_content(rsasl_err_to_str(self.e)
                .unwrap_or("Unknown gsasl error"));
        }

        Promise::ok(())
    }

    fn available(
        &mut self,
        _: l10n_string::AvailableParams,
        mut results: l10n_string::AvailableResults
    ) -> Promise<(), Error> {
        let builder = results.get();
        let mut langs = builder.init_langs(1);
        langs.set(0, "en");
        Promise::ok(())
    }
}