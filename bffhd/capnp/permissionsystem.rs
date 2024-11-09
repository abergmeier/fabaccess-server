use crate::Roles;
use api::permissionsystem_capnp::permission_system::info::{
    GetRoleListParams, GetRoleListResults, Server as PermissionSystem,
};
use capnp::capability::Promise;
use capnp::Error;
use tracing::Span;

use crate::session::SessionHandle;

const TARGET: &str = "bffh::api::permissionsystem";

pub struct Permissions {
    span: Span,
    roles: Roles,
}

impl Permissions {
    pub fn new(session: SessionHandle) -> Self {
        let span = tracing::info_span!(target: TARGET, "PermissionSystem",);
        Self {
            span,
            roles: session.roles,
        }
    }
}

impl PermissionSystem for Permissions {
    fn get_role_list(
        &mut self,
        _: GetRoleListParams,
        mut results: GetRoleListResults,
    ) -> Promise<(), Error> {
        let _guard = self.span.enter();
        let _span = tracing::trace_span!(target: TARGET, "getRoleList",).entered();

        tracing::trace!("method call");
        let roles = self.roles.list().collect::<Vec<&String>>();
        let mut builder = results.get();
        let mut b = builder.init_role_list(roles.len() as u32);
        for (i, role) in roles.into_iter().enumerate() {
            let mut role_builder = b.reborrow().get(i as u32);
            role_builder.set_name(role);
        }
        tracing::trace!("method return");
        Promise::ok(())
    }
}
