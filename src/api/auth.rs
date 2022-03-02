//! Authentication subsystem
//!
//! Authorization is over in `access.rs`
//! Authentication using SASL

use std::sync::Arc;
use std::rc::Rc;
use std::cell::RefCell;
use std::io::Cursor;


use slog::Logger;

use serde::{Serialize, Deserialize};

use capnp::capability::{Promise};
use rsasl::callback::Callback;
use rsasl::error::SessionError;
use rsasl::mechname::Mechname;
use rsasl::property::{AuthId, Password};
use rsasl::SASL;
use rsasl::session::Step;
use rsasl::validate::{Validation, validations};

use crate::api::Session;

pub use crate::schema::authenticationsystem_capnp as auth_system;
use crate::db::Databases;

use crate::db::user::{Internal as UserDB, User};
use crate::db::access::AccessControl as AccessDB;

pub struct AppData {
    userdb: Arc<UserDB>,
}
pub struct SessionData {
    authz: Option<User>,
}

struct CB {
    userdb: Arc<UserDB>,
}
impl CB {
    pub fn new(userdb: Arc<UserDB>) -> Self {
        Self { userdb }
    }
}

impl Callback for CB {
    fn validate(&self, session: &mut rsasl::session::SessionData, validation: Validation, _mechanism: &Mechname) -> Result<(), SessionError> {
        let ret = match validation {
            validations::SIMPLE => {

                let authid = session
                    .get_property::<AuthId>()
                    .ok_or(SessionError::no_property::<AuthId>())?;

                let pass = session.get_property::<Password>()
                                  .ok_or(SessionError::no_property::<Password>())?;

                if let Some(opt) = self.userdb.login(authid.as_ref(), pass.as_bytes()).unwrap() {
                    return Ok(())
                }

                SessionError::AuthenticationFailure
            }
            _ => {
                SessionError::no_validate(validation)
            }
        };
        Err(ret)
    }
}

pub struct Auth {
    pub ctx: SASL,
    session: Rc<RefCell<Option<Session>>>,
    userdb: Arc<UserDB>,
    access: Arc<AccessDB>,
    log: Logger,
}

impl Auth {
    pub fn new(log: Logger, dbs: Databases, session: Rc<RefCell<Option<Session>>>) -> Self {
        let mut ctx = SASL::new();
        ctx.install_callback(Arc::new(CB::new(dbs.userdb.clone())));

        Self { log, ctx, session, userdb: dbs.userdb.clone(), access: dbs.access.clone() }
    }
}

use crate::schema::authenticationsystem_capnp::*;
impl authentication_system::Server for Auth {
    fn mechanisms(&mut self, 
        _: authentication_system::MechanismsParams,
        mut res: authentication_system::MechanismsResults
    ) -> Promise<(), capnp::Error> {
        /*let mechs = match self.ctx.server_mech_list() {
            Ok(m) => m,
            Err(e) => {
                return Promise::err(capnp::Error {
                    kind: capnp::ErrorKind::Failed,
                    description: format!("SASL Failure: {}", e),
                })
            },
        };

        let mechvec: Vec<&str> = mechs.iter().collect();

        let mut res_mechs = res.get().init_mechs(mechvec.len() as u32);
        for (i, m) in mechvec.into_iter().enumerate() {
            res_mechs.set(i as u32, m);
        }*/
        // For now, only PLAIN
        let mut res_mechs = res.get().init_mechs(1);
        res_mechs.set(0, "PLAIN");

        Promise::ok(())
    }

    // TODO: return Outcome instead of exceptions
    fn start(&mut self,
        params: authentication_system::StartParams,
        mut res: authentication_system::StartResults
    ) -> Promise<(), capnp::Error> {
        let req = pry!(pry!(params.get()).get_request());

        // Extract the MECHANISM the client wants to use and start a session.
        // Or fail at that and thrown an exception TODO: return Outcome
        let mech = pry!(req.get_mechanism());
        if pry!(req.get_mechanism()) != "PLAIN" {
                return Promise::err(capnp::Error {
                    kind: capnp::ErrorKind::Failed,
                    description: format!("Invalid SASL mech"),
                })
        }

        let mech = Mechname::new(mech.as_bytes()).unwrap();

        let mut session = match self.ctx.server_start(mech) {
            Ok(s) => s,
            Err(e) => 
                return Promise::err(capnp::Error {
                    kind: capnp::ErrorKind::Failed,
                    description: format!("SASL error: {}", e),
                }),
        };

        let mut out = Cursor::new(Vec::new());

        // If the client has provided initial data go use that
        use request::initial_response::Which;
        let step_res = match req.get_initial_response().which() {
            Err(capnp::NotInSchema(_)) => 
                return Promise::err(capnp::Error {
                    kind: capnp::ErrorKind::Failed,
                    description: "Initial data is badly formatted".to_string(),
                }),

            Ok(Which::None(_)) => {
                // FIXME: Actually this needs to indicate NO data instead of SOME data of 0 length
                session.step(Option::<&[u8]>::None, &mut out)
            }
            Ok(Which::Initial(data)) => {
                session.step(Some(pry!(data)), &mut out)
            }
        };

        // The step may either return an error, a success or the need for more data
        // TODO: Set the session user. Needs a lookup though <.>

        match step_res {
            Ok(Step::Done(b)) => {
                let user = session
                    .get_property::<AuthId>()
                    .and_then(|data| {
                        self.userdb.get_user(data.as_str()).unwrap()
                    })
                    .expect("Authentication returned OK but the given AuthId is invalid");

                let perms = pry!(self.access.collect_permrules(&user.data)
                    .map_err(|e| capnp::Error::failed(format!("AccessDB lookup failed: {}", e))));
                self.session.replace(Some(Session::new(
                    self.log.new(o!()),
                    user.id,
                    "".to_string(),
                    user.data.roles.into_boxed_slice(),
                    perms.into_boxed_slice()
                )));

                let mut outcome = pry!(res.get().get_response()).init_outcome();
                outcome.reborrow().set_result(response::Result::Successful);
                if let Some(data) = b {
                    outcome.init_additional_data().set_additional(&out.get_ref());
                }
                Promise::ok(())
            },
            Ok(Step::NeedsMore(b)) => {
                if b.is_some() {
                    pry!(res.get().get_response()).set_challence(&out.get_ref());
                }
                Promise::ok(())
            }
            Err(e) => {
                let mut outcome = pry!(res.get().get_response()).init_outcome();
                outcome.reborrow().set_result(response::Result::InvalidCredentials);
                let text = format!("{}", e);
                outcome.set_help_text(&text);
                Promise::ok(())
            }
        }
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
