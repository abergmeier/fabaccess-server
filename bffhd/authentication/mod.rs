use crate::users::Users;
use miette::{Context, IntoDiagnostic};
use std::sync::Arc;
use rsasl::callback::{CallbackError, Request, SessionCallback, SessionData};
use rsasl::mechanism::SessionError;
use rsasl::prelude::{Mechname, SASLConfig, SASLServer, Session};
use rsasl::property::AuthId;

use crate::authentication::fabfire::FabFireCardKey;

mod fabfire;

struct Callback {
    users: Users,
    span: tracing::Span,
}
impl Callback {
    pub fn new(users: Users) -> Self {
        let span = tracing::info_span!("SASL callback");
        Self { users, span }
    }
}
impl SessionCallback for Callback {
    fn callback(&self, session_data: &SessionData, context: &rsasl::callback::Context, request: &mut Request) -> Result<(), SessionError> {
        if let Some(authid) = context.get_ref::<AuthId>() {
            request.satisfy_with::<FabFireCardKey, _>(|| {
                let user = self.users.get_user(authid).ok_or(CallbackError::NoValue)?;
                let kv = user.userdata.kv.get("cardkey").ok_or(CallbackError::NoValue)?;
                let card_key = <[u8; 16]>::try_from(
                    hex::decode(kv).map_err(|_| CallbackError::NoValue)?,
                ).map_err(|_| CallbackError::NoValue)?;
                Ok(card_key)
            })?;
        }
        Ok(())
    }

    /*fn validate(
        &self,
        session: &mut SessionData,
        validation: Validation,
        _mechanism: &Mechname,
    ) -> Result<(), SessionError> {
        let span = tracing::info_span!(parent: &self.span, "validate");
        let _guard = span.enter();
        match validation {
            validations::SIMPLE => {
                let authnid = session
                    .get_property::<AuthId>()
                    .ok_or(SessionError::no_property::<AuthId>())?;
                tracing::debug!(authid=%authnid, "SIMPLE validation requested");

                if let Some(user) = self.users.get_user(authnid.as_str()) {
                    let passwd = session
                        .get_property::<Password>()
                        .ok_or(SessionError::no_property::<Password>())?;

                    if user
                        .check_password(passwd.as_bytes())
                        .map_err(|_e| SessionError::AuthenticationFailure)?
                    {
                        return Ok(());
                    } else {
                        tracing::warn!(authid=%authnid, "AUTH FAILED: bad password");
                    }
                } else {
                    tracing::warn!(authid=%authnid, "AUTH FAILED: no such user '{}'", authnid);
                }

                Err(SessionError::AuthenticationFailure)
            }
            _ => {
                tracing::error!(?validation, "Unimplemented validation requested");
                Err(SessionError::no_validate(validation))
            }
        }
    }*/
}

struct Inner {
    rsasl: Arc<SASLConfig>,
}
impl Inner {
    pub fn new(rsasl: Arc<SASLConfig>) -> Self {
        Self { rsasl }
    }
}

#[derive(Clone)]
pub struct AuthenticationHandle {
    inner: Inner,
}

impl AuthenticationHandle {
    pub fn new(userdb: Users) -> Self {
        let span = tracing::debug_span!("authentication");
        let _guard = span.enter();

        let config = SASLConfig::builder()
            .with_defaults()
            .with_callback(Callback::new(userdb))
            .unwrap();

        let mechs: Vec<&'static str> = SASLServer::new(config.clone())
            .get_available()
            .into_iter()
            .map(|m| m.mechanism.as_str())
            .collect();
        tracing::info!(available_mechs = mechs.len(), "initialized sasl backend");
        tracing::debug!(?mechs, "available mechs");

        Self {
            inner: Inner::new(config),
        }
    }

    pub fn start(&self, mechanism: &Mechname) -> miette::Result<Session> {
        Ok(SASLServer::new(self.inner.rsasl.clone())
            .start_suggested(mechanism)
            .into_diagnostic()
            .wrap_err("Failed to start a SASL authentication with the given mechanism")?)
    }

    pub fn list_available_mechs(&self) -> impl IntoIterator<Item = &Mechname> {
        SASLServer::new(self.inner.rsasl.clone())
            .get_available()
            .into_iter()
            .map(|m| m.mechanism)
    }
}
