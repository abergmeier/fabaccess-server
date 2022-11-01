use api::connection_capnp::bootstrap;
pub use api::connection_capnp::bootstrap::Client;
use std::fmt;
use std::fmt::{Formatter, Write};
use std::net::SocketAddr;

use crate::authentication::AuthenticationHandle;
use crate::capnp::authenticationsystem::Authentication;
use crate::session::SessionManager;
use capnp::capability::Promise;
use capnp_rpc::pry;
use rsasl::mechname::Mechname;
use tracing::Span;

/// Cap'n Proto API Handler
pub struct BootCap {
    peer_addr: SocketAddr,
    authentication: AuthenticationHandle,
    sessionmanager: SessionManager,
    span: Span,
}

impl BootCap {
    pub fn new(
        peer_addr: SocketAddr,
        authentication: AuthenticationHandle,
        sessionmanager: SessionManager,
        span: Span,
    ) -> Self {
        Self {
            peer_addr,
            authentication,
            sessionmanager,
            span,
        }
    }
}

impl bootstrap::Server for BootCap {
    fn get_a_p_i_version(
        &mut self,
        _: bootstrap::GetAPIVersionParams,
        _: bootstrap::GetAPIVersionResults,
    ) -> Promise<(), ::capnp::Error> {
        let _guard = self.span.enter();
        let _span = tracing::trace_span!(
            target: "bffh::api",
            "Bootstrap",
            method = "getAPIVersion",
        )
        .entered();
        tracing::trace!("method call");
        Promise::ok(())
    }

    fn get_server_release(
        &mut self,
        _: bootstrap::GetServerReleaseParams,
        mut result: bootstrap::GetServerReleaseResults,
    ) -> Promise<(), ::capnp::Error> {
        let _guard = self.span.enter();
        let _span = tracing::trace_span!(
            target: "bffh::api",
            "Bootstrap",
            method = "getServerRelease",
        )
        .entered();
        tracing::trace!("method call");

        let mut builder = result.get();
        builder.set_name("bffhd");
        builder.set_release(crate::env::VERSION);

        tracing::trace!(
            results.name = "bffhd",
            results.release = crate::env::VERSION,
            "method return"
        );
        Promise::ok(())
    }

    fn mechanisms(
        &mut self,
        _params: bootstrap::MechanismsParams,
        mut result: bootstrap::MechanismsResults,
    ) -> Promise<(), ::capnp::Error> {
        let _guard = self.span.enter();
        let _span = tracing::trace_span!(
            target: "bffh::api",
            "mechanisms",
        )
        .entered();
        tracing::trace!(target: "bffh::api", "method call");

        let builder = result.get();
        let mechs: Vec<_> = self
            .authentication
            .sess()
            .get_available()
            .into_iter()
            .map(|m| m.mechanism.as_str())
            .collect();
        let mut mechbuilder = builder.init_mechs(mechs.len() as u32);
        for (i, m) in mechs.iter().enumerate() {
            mechbuilder.set(i as u32, m);
        }

        struct DisMechs<'a>(Vec<&'a str>);
        impl fmt::Display for DisMechs<'_> {
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                f.write_char('[')?;
                let mut first = true;
                for mechanism in self.0.iter() {
                    if first {
                        first = false;
                        f.write_str(mechanism)?;
                    } else {
                        f.write_str(" ,")?;
                        f.write_str(mechanism)?;
                    }
                }
                f.write_char(']')?;
                Ok(())
            }
        }
        tracing::trace!(
            results.mechs = %DisMechs(mechs),
            "method return"
        );
        Promise::ok(())
    }

    fn create_session(
        &mut self,
        params: bootstrap::CreateSessionParams,
        mut result: bootstrap::CreateSessionResults,
    ) -> Promise<(), ::capnp::Error> {
        let _guard = self.span.enter();
        let _span = tracing::trace_span!(
            target: "bffh::api",
            "createSession",
        )
        .entered();

        let params = pry!(params.get());
        let mechanism: &str = pry!(params.get_mechanism());

        tracing::trace!(params.mechanism = mechanism, "method call");

        let mechname = Mechname::parse(mechanism.as_bytes());
        let auth = if let Ok(mechname) = mechname {
            if let Ok(session) = self.authentication.start(mechname) {
                Authentication::new(&self.span, mechname, session, self.sessionmanager.clone())
            } else {
                Authentication::invalid_mechanism()
            }
        } else {
            Authentication::invalid_mechanism()
        };

        tracing::trace!(
            results.authentication = %auth,
            "method return"
        );

        let mut builder = result.get();
        builder.set_authentication(capnp_rpc::new_client(auth));

        Promise::ok(())
    }
}
