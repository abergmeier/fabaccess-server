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
pub struct AuthenticationHandler {
    inner: Arc<Inner>,
}

impl AuthenticationHandler {
    pub fn new(rsasl: SASL) -> Self {
        Self { inner: Arc::new(Inner::new(rsasl)) }
    }

    pub fn start(&self, mechanism: &Mechname) -> Result<Session, SASLError> {
        self.inner.rsasl.server_start(mechanism)
    }
}