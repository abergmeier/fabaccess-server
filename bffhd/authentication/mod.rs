use crate::users::Users;
use rsasl::error::SessionError;
use rsasl::mechname::Mechname;
use rsasl::property::{AuthId, Password};
use rsasl::session::{Session, SessionData};
use rsasl::validate::{validations, Validation};
use rsasl::{Property, SASL};
use std::sync::Arc;

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
impl rsasl::callback::Callback for Callback {
    fn provide_prop(
        &self,
        session: &mut rsasl::session::SessionData,
        property: Property,
    ) -> Result<(), SessionError> {
        match property {
            fabfire::FABFIRECARDKEY => {
                let authcid = session.get_property_or_callback::<AuthId>()?;
                let user = self
                    .users
                    .get_user(authcid.unwrap().as_ref())
                    .ok_or(SessionError::AuthenticationFailure)?;
                let kv = user
                    .userdata
                    .kv
                    .get("cardkey")
                    .ok_or(SessionError::AuthenticationFailure)?;
                let card_key = <[u8; 16]>::try_from(
                    hex::decode(kv).map_err(|_| SessionError::AuthenticationFailure)?,
                )
                .map_err(|_| SessionError::AuthenticationFailure)?;
                session.set_property::<FabFireCardKey>(Arc::new(card_key));
                Ok(())
            }
            _ => Err(SessionError::NoProperty { property }),
        }
    }

    fn validate(
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
    }
}

struct Inner {
    rsasl: SASL,
}
impl Inner {
    pub fn new(rsasl: SASL) -> Self {
        Self { rsasl }
    }
}

#[derive(Clone)]
pub struct AuthenticationHandle {
    inner: Arc<Inner>,
}

impl AuthenticationHandle {
    pub fn new(userdb: Users) -> Self {
        let span = tracing::debug_span!("authentication");
        let _guard = span.enter();

        let mut rsasl = SASL::new();
        rsasl.install_callback(Arc::new(Callback::new(userdb)));

        let mechs: Vec<&'static str> = rsasl
            .server_mech_list()
            .into_iter()
            .map(|m| m.mechanism.as_str())
            .collect();
        tracing::info!(available_mechs = mechs.len(), "initialized sasl backend");
        tracing::debug!(?mechs, "available mechs");

        Self {
            inner: Arc::new(Inner::new(rsasl)),
        }
    }

    pub fn start(&self, mechanism: &Mechname) -> anyhow::Result<Session> {
        Ok(self.inner.rsasl.server_start(mechanism)?)
    }

    pub fn list_available_mechs(&self) -> impl IntoIterator<Item = &Mechname> {
        self.inner
            .rsasl
            .server_mech_list()
            .into_iter()
            .map(|m| m.mechanism)
    }
}
