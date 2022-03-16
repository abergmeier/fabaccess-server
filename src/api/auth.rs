//! Authentication subsystem
//!
//! Authorization is over in `access.rs`
//! Authentication using SASL

use std::cell::RefCell;
use std::convert::TryFrom;
use std::io::Cursor;
use std::rc::Rc;
use std::sync::Arc;

use slog::Logger;

use serde::{Deserialize, Serialize};

use crate::api::machines::Machines;
use capnp::capability::Promise;
use rsasl::callback::Callback;
use rsasl::error::SessionError;
use rsasl::mechname::Mechname;
use rsasl::property::{AuthId, Password};
use rsasl::{Property, SASL};
use rsasl::session::Session as RsaslSession;
use rsasl::session::Step;
use rsasl::validate::{validations, Validation};

use crate::api::users::Users;
use crate::api::Session;

use crate::db::Databases;
pub use crate::schema::authenticationsystem_capnp as auth_system;

use crate::db::access::AccessControl as AccessDB;
use crate::db::user::{Internal as UserDB, User, UserId};
use crate::network::Network;

mod fabfire;
pub use fabfire::FABFIRE;
use crate::api::auth::fabfire::FabFireCardKey;

pub struct AppData {
    userdb: Arc<UserDB>,
}
pub struct SessionData {
    authz: Option<User>,
}

pub struct CB {
    userdb: Arc<UserDB>,
}
impl CB {
    pub fn new(userdb: Arc<UserDB>) -> Self {
        Self { userdb }
    }
}

impl Callback for CB {
    fn validate(
        &self,
        session: &mut rsasl::session::SessionData,
        validation: Validation,
        _mechanism: &Mechname,
    ) -> Result<(), SessionError> {
        let ret = match validation {
            validations::SIMPLE => {

                let authid = session
                    .get_property::<AuthId>()
                    .ok_or(SessionError::no_property::<AuthId>())?;

                let pass = session
                    .get_property::<Password>()
                    .ok_or(SessionError::no_property::<Password>())?;

                if self
                    .userdb
                    .login(authid.as_ref(), pass.as_bytes())
                    .unwrap()
                    .is_some()
                {
                    return Ok(());
                }

                SessionError::AuthenticationFailure
            }
            _ => SessionError::no_validate(validation),
        };
        Err(ret)
    }

    fn provide_prop(
        &self,
        session: &mut rsasl::session::SessionData,
        property: Property,
    ) -> Result<(), SessionError> {
        match property {
            fabfire::FABFIRECARDKEY => {
                let authcid = session.get_property_or_callback::<AuthId>()?;
                self.userdb.get_user(authcid.unwrap().as_ref()).map(|user| {
                    let kvs= user.unwrap().data.kv;
                    kvs.get("cardkey").map(|key| {
                        session.set_property::<FabFireCardKey>(Arc::new(<[u8; 16]>::try_from(hex::decode(key).unwrap()).unwrap()));
                    });
                }).ok();

                Ok(())
            }
            _ => Err(SessionError::NoProperty { property }),
        }
    }
}

pub enum State {
    InvalidMechanism,
    Finished,
    Aborted,
    Running(RsaslSession),
}

pub struct Auth {
    userdb: Arc<UserDB>,
    access: Arc<AccessDB>,
    state: State,
    log: Logger,
    network: Arc<Network>,
}

impl Auth {
    pub fn new(log: Logger, dbs: Databases, state: State, network: Arc<Network>) -> Self {
        Self {
            log,
            userdb: dbs.userdb.clone(),
            access: dbs.access.clone(),
            state,
            network,
        }
    }

    fn build_error(&self, response: response::Builder) {
        use crate::schema::authenticationsystem_capnp::response::Error as ErrorCode;
        if let State::Running(_) = self.state {
            return;
        }

        let mut builder = response.init_failed();
        match self.state {
            State::InvalidMechanism => builder.set_code(ErrorCode::BadMechanism),
            State::Finished => builder.set_code(ErrorCode::Aborted),
            State::Aborted => builder.set_code(ErrorCode::Aborted),
            _ => unreachable!(),
        }
    }
}

use crate::schema::authenticationsystem_capnp::*;
impl authentication::Server for Auth {
    fn step(
        &mut self,
        params: authentication::StepParams,
        mut results: authentication::StepResults,
    ) -> Promise<(), capnp::Error> {
        let mut builder = results.get();
        if let State::Running(mut session) = std::mem::replace(&mut self.state, State::Aborted) {
            let data: &[u8] = pry!(pry!(params.get()).get_data());
            let mut out = Cursor::new(Vec::new());
            match session.step(Some(data), &mut out) {
                Ok(Step::Done(data)) => {
                    trace!(self.log, "Authentication done!");
                    self.state = State::Finished;

                    let uid = pry!(session.get_property::<AuthId>().ok_or(capnp::Error::failed(
                        "Authentication didn't provide an authid as required".to_string()
                    )));
                    let user = self.userdb.get_user(uid.as_str()).unwrap()
                        .expect("Just auth'ed user was not found?!");

                    let mut builder = builder.init_successful();
                    if data.is_some() {
                        builder.set_additional_data(out.into_inner().as_slice());
                    }

                    let mut builder = builder.init_session();
                    let perms = pry!(self.access.collect_permrules(&user.data)
                        .map_err(|e| capnp::Error::failed(format!("AccessDB lookup failed: {}", e))));

                    let session = Rc::new(RefCell::new(Some(Session::new(
                        self.log.clone(),
                        user.id,
                        uid.to_string(),
                        user.data.roles.into_boxed_slice(),
                        perms.into_boxed_slice(),
                    ))));

                    builder.set_machine_system(capnp_rpc::new_client(Machines::new(
                        session.clone(),
                        self.network.clone(),
                    )));
                    builder.set_user_system(capnp_rpc::new_client(Users::new(
                        session.clone(),
                        self.userdb.clone(),
                    )));
                }
                Ok(Step::NeedsMore(_)) => {
                    trace!(self.log, "Authentication wants more data");
                    builder.set_challenge(&out.get_ref());
                    self.state = State::Running(session);
                }
                Err(error) => {
                    trace!(self.log, "Authentication errored: {}", error);
                    self.state = State::Aborted;
                    self.build_error(builder);
                }
            }
        } else {
            self.build_error(builder);
        }

        Promise::ok(())
    }

    fn abort(
        &mut self,
        _: authentication::AbortParams,
        _: authentication::AbortResults,
    ) -> Promise<(), capnp::Error> {
        self.state = State::Aborted;
        Promise::ok(())
    }
}

// Use the newtype pattern here to make the type system work for us; even though AuthCId is for all
// intents and purposes just a String the compiler will still complain if you return or more
// importantly pass a String intead of a AuthCId. This prevents bugs where you get an object from
// somewhere and pass it somewhere else and in between don't check if it's the right type and
// accidentally pass the authzid where the authcid should have gone.

// What is a man?! A miserable little pile of secrets!
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
/// Authentication/Authorization user object.
///
/// This struct describes the user as can be gathered from API authentication exchanges.
/// Specifically this is the value bffh gets after a successful authentication.
///
pub struct AuthenticationData {
    /// Contains the Authentication ID used
    ///
    /// The authentication ID is an identifier for the authentication exchange. This is
    /// conceptually different than the ID of the user to be authenticated; for example when using
    /// x509 the authcid is the dn of the certificate, when using GSSAPI the authcid is of form
    /// `<ID>@<REALM>`
    authcid: String,

    /// Authorization ID
    ///
    /// The authzid represents the identity that a client wants to act as. In our case this is
    /// always an user id. If unset no preference is indicated and the server will authenticate the
    /// client as whatever user — if any — they associate with the authcid. Setting the authzid is
    /// useful in a number if situations:
    /// If somebody wants to authenticate as somebody else, su-style.
    /// If a person wants to authenticate as a higher-permissions account, e.g. foo may set authzid foo+admin
    /// to split normal user and "admin" accounts.
    /// If a method requires a specific authcid that is different from the identifier of the user
    /// to authenticate as, e.g. GSSAPI, x509 client certificates, API TOKEN authentication.
    authzid: String,

    /// Contains the authentication method used
    ///
    /// For the most part this is the SASL method
    auth_method: String,

    /// Method-specific key-value pairs
    ///
    /// Each method can use their own key-value pairs.
    /// E.g. EXTERNAL encodes the actual method used (x509 client certs, UID/GID for unix sockets,
    /// ...)
    kvs: Box<[(String, String)]>,
}

// Authentication has two parts: Granting the authentication itself and then performing the
// authentication.
// Granting the authentication checks if
// a) the given authcid fits with the given (authMethod, kvs). In general a failure here indicates
//    a programming failure — the authcid come from the same source as that tuple
// b) the given authcid may authenticate as the given authzid. E.g. if a given client certificate
//    has been configured for that user, if a GSSAPI user maps to a given user,

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AuthError {
    /// Authentication ID is bad/unknown/..
    BadAuthcid,
    /// Authorization ID is unknown/..
    BadAuthzid,
    /// Authorization ID is not of form user+uid@realm
    MalformedAuthzid,
    /// User may not use that authorization id
    NotAllowedAuthzid,
}
