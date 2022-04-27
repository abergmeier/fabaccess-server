use capnp::capability::Promise;
use capnp_rpc::pry;
use crate::session::SessionHandle;
use api::user_capnp::user::{admin, info, manage, self};
use api::general_capnp::optional;
use crate::authorization::permissions::Permission;
use crate::users::{db, UserRef};

#[derive(Clone)]
pub struct User {
    session: SessionHandle,
    user: UserRef,
}

impl User {
    pub fn new(session: SessionHandle, user: UserRef) -> Self {
        Self { session, user }
    }

    pub fn new_self(session: SessionHandle) -> Self {
        let user = session.get_user_ref();
        Self::new(session, user)
    }

    pub fn build_optional(&self, user: Option<UserRef>, builder: optional::Builder<user::Owned>) {
        if let Some(user) = user.and_then(|u| self.session.users.get_user(u.get_username())) {
            let builder = builder.init_just();
            Self::fill(&self.session, user, builder);
        }
    }

    pub fn build(session: SessionHandle, builder: user::Builder) {
        let this = Self::new_self(session);
        let user = this.session.get_user();
        Self::fill(&this.session, user, builder);
    }

    pub fn fill(session: &SessionHandle, user: db::User, mut builder: user::Builder) {
        builder.set_username(user.id.as_str());

        // We have permissions on ourself
        let is_me = &session.get_user_ref().id == &user.id;

        let client = Self::new(session.clone(), UserRef::new(user.id));

        if is_me || session.has_perm(Permission::new("bffh.users.info")) {
            builder.set_info(capnp_rpc::new_client(client.clone()));
        }
        if is_me {
            builder.set_manage(capnp_rpc::new_client(client.clone()));
        }
        if session.has_perm(Permission::new("bffh.users.admin")) {
            builder.set_admin(capnp_rpc::new_client(client.clone()));
        }
    }
}

impl info::Server for User {
    fn list_roles(
        &mut self,
        _: info::ListRolesParams,
        mut result: info::ListRolesResults,
    ) -> Promise<(), ::capnp::Error> {
        if let Some(user) = self.session.users.get_user(self.user.get_username()) {
            let mut builder = result.get().init_roles(user.userdata.roles.len() as u32);
            for (i, role) in user.userdata.roles.into_iter().enumerate() {
                let mut b = builder.reborrow().get(i as u32);
                b.set_name(role.as_str());
            }
        }
        Promise::ok(())
    }
}

impl manage::Server for User {
    fn pwd(
        &mut self,
        _params: manage::PwdParams,
        _results: manage::PwdResults,
    ) -> Promise<(), ::capnp::Error> {
        Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
}

impl admin::Server for User {
    fn get_user_info_extended(
        &mut self,
        _: admin::GetUserInfoExtendedParams,
        _: admin::GetUserInfoExtendedResults,
    ) -> Promise<(), ::capnp::Error> {
        Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
    fn add_role(
        &mut self,
        param: admin::AddRoleParams,
        _: admin::AddRoleResults,
    ) -> Promise<(), ::capnp::Error> {
        let rolename = pry!(pry!(pry!(param.get()).get_role()).get_name());

        if let Some(_role) = self.session.roles.get(rolename) {
            let mut target = self.session.users.get_user(self.user.get_username()).unwrap();

            // Only update if needed
            if !target.userdata.roles.iter().any(|r| r.as_str() == rolename) {
                target.userdata.roles.push(rolename.to_string());
                self.session.users.put_user(self.user.get_username(), &target);
            }
        }

        Promise::ok(())
    }
    fn remove_role(
        &mut self,
        param: admin::RemoveRoleParams,
        _: admin::RemoveRoleResults,
    ) -> Promise<(), ::capnp::Error> {
        let rolename = pry!(pry!(pry!(param.get()).get_role()).get_name());

        if let Some(_role) = self.session.roles.get(rolename) {
            let mut target = self.session.users.get_user(self.user.get_username()).unwrap();

            // Only update if needed
            if target.userdata.roles.iter().any(|r| r.as_str() == rolename) {
                target.userdata.roles.retain(|r| r.as_str() != rolename);
                self.session.users.put_user(self.user.get_username(), &target);
            }
        }

        Promise::ok(())
    }
    fn pwd(
        &mut self,
        _: admin::PwdParams,
        _: admin::PwdResults,
    ) -> Promise<(), ::capnp::Error> {
        Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
}
