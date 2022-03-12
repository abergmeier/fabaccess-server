use std::sync::Arc;
use rsasl::error::SASLError;
use rsasl::mechname::Mechname;
use rsasl::SASL;
use rsasl::session::Session;

pub mod db;

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
    pub fn new() -> Self {
        let rsasl = SASL::new();
        Self { inner: Arc::new(Inner::new(rsasl)) }
    }

    pub fn start(&self, mechanism: &Mechname) -> anyhow::Result<Session> {
        Ok(self.inner.rsasl.server_start(mechanism)?)
    }

    pub fn list_available_mechs(&self) -> impl IntoIterator<Item=&Mechname> {
        self.inner.rsasl.server_mech_list().into_iter().map(|m| m.mechanism)
    }
}