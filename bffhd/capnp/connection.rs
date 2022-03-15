use std::net::SocketAddr;
pub use api::connection_capnp::bootstrap::Client;
use api::connection_capnp::bootstrap;

use capnp::capability::Promise;
use capnp_rpc::pry;
use rsasl::mechname::Mechname;
use crate::authentication::AuthenticationHandle;
use crate::capnp::authenticationsystem::Authentication;
use crate::session::SessionManager;

/// Cap'n Proto API Handler
pub struct BootCap {
    peer_addr: SocketAddr,
    authentication: AuthenticationHandle,
    sessionmanager: SessionManager,
}

impl BootCap {
    pub fn new(peer_addr: SocketAddr, authentication: AuthenticationHandle, sessionmanager: SessionManager) -> Self {
        tracing::trace!(%peer_addr, "bootstrapping RPC");
        Self {
            peer_addr,
            authentication,
            sessionmanager,
        }
    }
}

impl bootstrap::Server for BootCap {
    fn get_a_p_i_version(
        &mut self,
        _: bootstrap::GetAPIVersionParams,
        _: bootstrap::GetAPIVersionResults,
    ) -> Promise<(), ::capnp::Error> {
        let span = tracing::trace_span!("get_api_version", peer_addr=%self.peer_addr);
        let _guard = span.enter();
        Promise::ok(())
    }

    fn get_server_release(
        &mut self,
        _: bootstrap::GetServerReleaseParams,
        mut result: bootstrap::GetServerReleaseResults,
    ) -> Promise<(), ::capnp::Error> {
        let span = tracing::trace_span!("get_server_release", peer_addr=%self.peer_addr);
        let _guard = span.enter();
        let mut builder = result.get();
        builder.set_name("bffhd");
        builder.set_release(crate::RELEASE_STRING);
        Promise::ok(())
    }

    fn mechanisms(
        &mut self,
        _params: bootstrap::MechanismsParams,
        mut result: bootstrap::MechanismsResults,
    ) -> Promise<(), ::capnp::Error> {
        let span = tracing::trace_span!("mechanisms", peer_addr=%self.peer_addr);
        let _guard = span.enter();

        tracing::trace!("mechanisms");

        let builder = result.get();
        let mechs: Vec<_> = self.authentication.list_available_mechs()
            .into_iter()
            .map(|m| m.as_str())
            .collect();
        let mut mechbuilder = builder.init_mechs(mechs.len() as u32);
        for (i,m) in mechs.iter().enumerate() {
            mechbuilder.set(i as u32, m);
        }

        Promise::ok(())
    }

    fn create_session(
        &mut self,
        params: bootstrap::CreateSessionParams,
        mut result: bootstrap::CreateSessionResults,
    ) -> Promise<(), ::capnp::Error> {
        let span = tracing::trace_span!("create_session", peer_addr=%self.peer_addr);
        let _guard = span.enter();

        let params = pry!(params.get());
        let mechanism: &str = pry!(params.get_mechanism());

        tracing::trace!(mechanism);

        let mechname = Mechname::new(mechanism.as_bytes());
        let auth = if let Ok(mechname) = mechname {
            if let Ok(session) = self.authentication.start(mechname) {
                Authentication::new(session, self.sessionmanager.clone())
            } else {
                Authentication::invalid_mechanism()
            }
        } else {
            Authentication::invalid_mechanism()
        };

        let mut builder = result.get();
        builder.set_authentication(capnp_rpc::new_client(auth));

        Promise::ok(())
    }
}
