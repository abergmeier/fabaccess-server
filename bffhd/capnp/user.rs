use crate::authorization::permissions::Permission;
use crate::session::SessionHandle;
use crate::users::{db, UserRef};
use crate::CONFIG;
use api::general_capnp::optional;
use api::user_capnp::user::card_d_e_s_fire_e_v2::{
    BindParams, BindResults, GenCardTokenParams, GenCardTokenResults, GetMetaInfoParams,
    GetMetaInfoResults, GetSpaceInfoParams, GetSpaceInfoResults, GetTokenListParams,
    GetTokenListResults, UnbindParams, UnbindResults,
};
use api::user_capnp::user::{self, admin, card_d_e_s_fire_e_v2, info, manage};
use capnp::capability::Promise;
use capnp::Error;
use capnp_rpc::pry;
use std::borrow::Cow;
use std::io::Write;
use uuid::Uuid;

const TARGET: &str = "bffh::api::user";

#[derive(Clone)]
pub struct User {
    span: tracing::Span,
    session: SessionHandle,
    user: UserRef,
}

impl User {
    pub fn new(session: SessionHandle, user: UserRef) -> Self {
        let span = tracing::info_span!(target: TARGET, "User");
        Self {
            span,
            session,
            user,
        }
    }

    pub fn new_self(session: SessionHandle) -> Self {
        let user = session.get_user_ref();
        Self::new(session, user)
    }

    pub fn build_optional(
        session: &SessionHandle,
        user: Option<UserRef>,
        builder: optional::Builder<user::Owned>,
    ) {
        if let Some(user) = user.and_then(|u| session.users.get_user(u.get_username())) {
            let builder = builder.init_just();
            Self::fill(&session, user, builder);
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
            builder.set_card_d_e_s_fire_e_v2(capnp_rpc::new_client(client));
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
        params: manage::PwdParams,
        _results: manage::PwdResults,
    ) -> Promise<(), ::capnp::Error> {
        let params = pry!(params.get());
        let old_pw = pry!(params.get_old_pwd());
        let new_pw = pry!(params.get_new_pwd());

        let uid = self.user.get_username();
        if let Some(mut user) = self.session.users.get_user(uid) {
            if let Ok(true) = user.check_password(old_pw.as_bytes()) {
                user.set_pw(new_pw.as_bytes());
                self.session.users.put_user(uid, &user);
            }
        }
        Promise::ok(())
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
            let mut target = self
                .session
                .users
                .get_user(self.user.get_username())
                .unwrap();

            // Only update if needed
            if !target.userdata.roles.iter().any(|r| r.as_str() == rolename) {
                target.userdata.roles.push(rolename.to_string());
                self.session
                    .users
                    .put_user(self.user.get_username(), &target);
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
            let mut target = self
                .session
                .users
                .get_user(self.user.get_username())
                .unwrap();

            // Only update if needed
            if target.userdata.roles.iter().any(|r| r.as_str() == rolename) {
                target.userdata.roles.retain(|r| r.as_str() != rolename);
                self.session
                    .users
                    .put_user(self.user.get_username(), &target);
            }
        }

        Promise::ok(())
    }
    fn pwd(
        &mut self,
        param: admin::PwdParams,
        _: admin::PwdResults,
    ) -> Promise<(), ::capnp::Error> {
        let new_pw = pry!(pry!(param.get()).get_new_pwd());
        let uid = self.user.get_username();
        if let Some(mut user) = self.session.users.get_user(uid) {
            user.set_pw(new_pw.as_bytes());
            self.session.users.put_user(uid, &user);
        }
        Promise::ok(())
    }
}

impl card_d_e_s_fire_e_v2::Server for User {
    fn get_token_list(
        &mut self,
        _: GetTokenListParams,
        mut results: GetTokenListResults,
    ) -> Promise<(), Error> {
        let _guard = self.span.enter();
        let _span = tracing::trace_span!(target: TARGET, "get_token_list").entered();
        tracing::trace!("method call");

        // TODO: This only supports a sigle key per user
        let user = pry!(self
            .session
            .users
            .get_user(self.user.get_username())
            .ok_or_else(|| Error::failed(format!(
                "User API object with nonexisting user \"{}\"",
                self.user.get_username()
            ))));
        let ck = user
            .userdata
            .kv
            .get("cardkey")
            .map(|ck| hex::decode(ck).ok())
            .flatten()
            .unwrap_or_else(|| {
                tracing::debug!(user.id = &user.id, "no DESFire keys stored");
                Vec::new()
            });
        if !ck.is_empty() {
            let mut b = results.get();
            let mut lb = b.init_token_list(1);
            lb.set(0, &ck[..]);
        }
        Promise::ok(())
    }

    fn bind(&mut self, params: BindParams, _: BindResults) -> Promise<(), Error> {
        let _guard = self.span.enter();
        let _span = tracing::trace_span!(target: TARGET, "get_token_list").entered();
        let params = pry!(params.get());
        let card_key = pry!(params.get_auth_key());
        let token = pry!(params.get_token());

        let token: Cow<'_, str> = if let Ok(url) = std::str::from_utf8(token) {
            Cow::Borrowed(url)
        } else {
            Cow::Owned(hex::encode(token))
        };

        tracing::trace!(
            params.token = token.as_ref(),
            params.auth_key = "<censored>",
            "method call"
        );

        let card_key = hex::encode(card_key);

        let mut user = pry!(self
            .session
            .users
            .get_user(self.user.get_username())
            .ok_or_else(|| Error::failed(format!(
                "User API object with nonexisting user \"{}\"",
                self.user.get_username()
            ))));

        let prev_token = user.userdata.kv.get("cardtoken");
        let prev_cardk = user.userdata.kv.get("cardkey");

        match (prev_token, prev_cardk) {
            (Some(prev_token), Some(prev_cardk))
                if prev_token.as_str() == &token && prev_cardk.as_str() == card_key.as_str() =>
            {
                tracing::info!(
                    user.id, token = token.as_ref(),
                    "new token and card key are identical, skipping no-op"
                );
                return Promise::ok(());
            },
            (Some(prev_token), Some(_))
                if prev_token.as_str() == token /* above guard means prev_cardk != card_key */ =>
            {
                tracing::warn!(
                    token = token.as_ref(),
                    "trying to overwrite card key for existing token, ignoring!"
                );
                return Promise::ok(());
            },
            (Some(prev_token), None) => tracing::warn!(
                user.id, prev_token,
                "token already set for user but no card key, setting new pair unconditionally!"
            ),
            (None, Some(_)) => tracing::warn!(
                user.id,
                "card key already set for user but no token, setting new pair unconditionally!"
            ),
            (Some(_), Some(_)) | (None, None) => tracing::debug!(
                user.id, token = token.as_ref(),
                "Adding new card key/token pair"
            ),
        }

        user.userdata
            .kv
            .insert("cardtoken".to_string(), token.to_string());
        user.userdata.kv.insert("cardkey".to_string(), card_key);
        Promise::ok(())
    }

    fn unbind(&mut self, params: UnbindParams, _: UnbindResults) -> Promise<(), Error> {
        let _guard = self.span.enter();
        let _span = tracing::trace_span!(target: TARGET, "get_token_list").entered();

        let params = pry!(params.get());
        let token = pry!(params.get_token());

        let token: Cow<'_, str> = if let Ok(url) = std::str::from_utf8(token) {
            Cow::Borrowed(url)
        } else {
            Cow::Owned(hex::encode(token))
        };

        tracing::trace!(params.token = token.as_ref(), "method call");

        let mut user = pry!(self
            .session
            .users
            .get_user(self.user.get_username())
            .ok_or_else(|| Error::failed(format!(
                "User API object with nonexisting user \"{}\"",
                self.user.get_username()
            ))));
        if let Some(prev_token) = user.userdata.kv.get("cardtoken") {
            if token.as_ref() == prev_token.as_str() {
                user.userdata.kv.remove("cardtoken");
                user.userdata.kv.remove("cardkey");
            }
        }

        Promise::ok(())
    }

    fn gen_card_token(
        &mut self,
        _: GenCardTokenParams,
        mut results: GenCardTokenResults,
    ) -> Promise<(), Error> {
        let _guard = self.span.enter();
        let _span = tracing::trace_span!(target: TARGET, "get_card_token").entered();
        tracing::trace!("method call");

        results.get().set_token(Uuid::new_v4().as_bytes());

        Promise::ok(())
    }

    fn get_meta_info(
        &mut self,
        _: GetMetaInfoParams,
        mut results: GetMetaInfoResults,
    ) -> Promise<(), Error> {
        let _guard = self.span.enter();
        let _span = tracing::trace_span!(target: TARGET, "get_meta_info").entered();
        tracing::trace!("method call");

        results.get().set_bytes(b"FABACCESS\x00DESFIRE\x001.0\x00");

        Promise::ok(())
    }

    fn get_space_info(
        &mut self,
        _: GetSpaceInfoParams,
        mut results: GetSpaceInfoResults,
    ) -> Promise<(), Error> {
        let _guard = self.span.enter();
        let _span = tracing::trace_span!(target: TARGET, "get_space_info").entered();
        tracing::trace!("method call");

        let space = if let Some(space) = CONFIG.get().and_then(|c| c.spacename.as_ref()) {
            space
        } else {
            return Promise::err(Error::failed("No space name configured".to_string()));
        };

        let url = if let Some(url) = CONFIG.get().and_then(|c| c.instanceurl.as_ref()) {
            url
        } else {
            return Promise::err(Error::failed("No instance url configured".to_string()));
        };

        let mut data = Vec::new();
        write!(&mut data, "urn:fabaccess:lab:{space}\x00{url}").unwrap();
        results.get().set_bytes(&data);

        Promise::ok(())
    }
}
