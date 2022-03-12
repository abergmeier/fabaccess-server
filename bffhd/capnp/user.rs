use api::permissionsystem_capnp::permission_system::Server as PermissionSystem;
use api::user_capnp::user::{
    info,
    manage,
    admin,
};

struct User;

impl info::Server for User {

    fn list_roles(&mut self, _: info::ListRolesParams<>, _: info::ListRolesResults<>) -> ::capnp::capability::Promise<(), ::capnp::Error> { ::capnp::capability::Promise::err(::capnp::Error::unimplemented("method not implemented".to_string())) }
}

impl manage::Server for User {

    fn pwd(&mut self, _: manage::PwdParams<>, _: manage::PwdResults<>) -> ::capnp::capability::Promise<(), ::capnp::Error> { ::capnp::capability::Promise::err(::capnp::Error::unimplemented("method not implemented".to_string())) }
}

impl admin::Server for User {

    fn get_user_info_extended(&mut self, _: admin::GetUserInfoExtendedParams<>, _: admin::GetUserInfoExtendedResults<>) -> ::capnp::capability::Promise<(), ::capnp::Error> { ::capnp::capability::Promise::err(::capnp::Error::unimplemented("method not implemented".to_string())) }
    fn add_role(&mut self, _: admin::AddRoleParams<>, _: admin::AddRoleResults<>) -> ::capnp::capability::Promise<(), ::capnp::Error> { ::capnp::capability::Promise::err(::capnp::Error::unimplemented("method not implemented".to_string())) }
    fn remove_role(&mut self, _: admin::RemoveRoleParams<>, _: admin::RemoveRoleResults<>) -> ::capnp::capability::Promise<(), ::capnp::Error> { ::capnp::capability::Promise::err(::capnp::Error::unimplemented("method not implemented".to_string())) }
    fn pwd(&mut self, _: admin::PwdParams<>, _: admin::PwdResults<>) -> ::capnp::capability::Promise<(), ::capnp::Error> { ::capnp::capability::Promise::err(::capnp::Error::unimplemented("method not implemented".to_string())) }
}