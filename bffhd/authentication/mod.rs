use crate::users::db::UserDB;
use crate::users::Users;
use rsasl::error::{SASLError, SessionError};
use rsasl::mechname::Mechname;
use rsasl::property::{AuthId, Password};
use rsasl::session::{Session, SessionData};
use rsasl::validate::{validations, Validation};
use rsasl::{Property, SASL};
use std::sync::Arc;

pub mod db;

struct Callback {
    users: Users,
}
impl Callback {
    pub fn new(users: Users) -> Self {
        Self { users }
    }
}
impl rsasl::callback::Callback for Callback {
    fn validate(
        &self,
        session: &mut SessionData,
        validation: Validation,
        mechanism: &Mechname,
    ) -> Result<(), SessionError> {
        match validation {
            validations::SIMPLE => {
                let authnid = session
                    .get_property::<AuthId>()
                    .ok_or(SessionError::no_property::<AuthId>())?;
                let user = self
                    .users
                    .get_user(authnid.as_str())
                    .ok_or(SessionError::AuthenticationFailure)?;
                let passwd = session
                    .get_property::<Password>()
                    .ok_or(SessionError::no_property::<Password>())?;

                if user
                    .check_password(passwd.as_bytes())
                    .map_err(|e| SessionError::AuthenticationFailure)?
                {
                    Ok(())
                } else {
                    Err(SessionError::AuthenticationFailure)
                }
            }
            _ => Err(SessionError::no_validate(validation)),
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
        let mut rsasl = SASL::new();
        rsasl.install_callback(Arc::new(Callback::new(userdb)));
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
