use crate::users::Users;
use miette::{IntoDiagnostic, WrapErr};
use std::sync::Arc;
use rsasl::callback::{CallbackError, Request, SessionCallback, SessionData, Context};
use rsasl::mechanism::SessionError;
use rsasl::prelude::{Mechname, SASLConfig, SASLServer, Session, Validation};
use rsasl::property::{AuthId, AuthzId, Password};
use rsasl::validate::{Validate, ValidationError};

use crate::authentication::fabfire::FabFireCardKey;
use crate::users::db::User;

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
    fn callback(&self, session_data: &SessionData, context: &Context, request: &mut Request) -> Result<(), SessionError> {
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

    fn validate(&self, session_data: &SessionData, context: &Context, validate: &mut Validate<'_>) -> Result<(), ValidationError> {
        let span = tracing::info_span!(parent: &self.span, "validate");
        let _guard = span.enter();
        if validate.is::<V>() {
            match session_data.mechanism().mechanism.as_str() {
                "PLAIN" => {
                    let authcid = context.get_ref::<AuthId>()
                        .ok_or(ValidationError::MissingRequiredProperty)?;
                    let authzid = context.get_ref::<AuthzId>();
                    let password = context.get_ref::<Password>()
                        .ok_or(ValidationError::MissingRequiredProperty)?;

                    if authzid.is_some() {
                        return Ok(())
                    }

                    if let Some(user) = self.users.get_user(authcid) {
                        match user.check_password(password) {
                            Ok(true) => {
                                validate.finalize::<V>(user)
                            }
                            Ok(false) => {
                                tracing::warn!(authid=%authcid, "AUTH FAILED: bad password");
                            }
                            Err(error) => {
                                tracing::warn!(authid=%authcid, "Bad DB entry: {}", error);
                            }
                        }
                    } else {
                        tracing::warn!(authid=%authcid, "AUTH FAILED: no such user");
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

pub struct V;
impl Validation for V {
    type Value = User;
}

#[derive(Clone)]
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

        let mechs: Vec<&'static str> = SASLServer::<V>::new(config.clone())
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

    pub fn start(&self, mechanism: &Mechname) -> miette::Result<Session<V>> {
        Ok(SASLServer::new(self.inner.rsasl.clone())
            .start_suggested(mechanism)
            .into_diagnostic()
            .wrap_err("Failed to start a SASL authentication with the given mechanism")?)
    }

    pub fn sess(&self) -> SASLServer<V> {
        SASLServer::new(self.inner.rsasl.clone())
    }
}
