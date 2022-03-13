use std::sync::Arc;
use rsasl::error::{SASLError, SessionError};
use rsasl::mechname::Mechname;
use rsasl::{Property, SASL};
use rsasl::session::{Session, SessionData};
use rsasl::validate::Validation;
use crate::users::db::UserDB;
use crate::users::Users;

pub mod db;

struct Callback {
    users: Users,
}
impl Callback {
    pub fn new(users: Users) -> Self {
        Self { users, }
    }
}
impl rsasl::callback::Callback for Callback {
    fn validate(&self, session: &mut SessionData, validation: Validation, mechanism: &Mechname) -> Result<(), SessionError> {
        todo!()
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
        Self { inner: Arc::new(Inner::new(rsasl)) }
    }

    pub fn start(&self, mechanism: &Mechname) -> anyhow::Result<Session> {
        Ok(self.inner.rsasl.server_start(mechanism)?)
    }

    pub fn list_available_mechs(&self) -> impl IntoIterator<Item=&Mechname> {
        self.inner.rsasl.server_mech_list().into_iter().map(|m| m.mechanism)
    }
}