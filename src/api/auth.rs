//! Authentication subsystem
//!
//! Authorization is over in `access.rs`
//! Authentication using SASL

use std::sync::Arc;

use rsasl::{
    SASL,
    Property,
    Session as SaslSession,
    ReturnCode,
    Callback,
    SaslCtx,
    Step,
};

use serde::{Serialize, Deserialize};

use capnp::capability::{Params, Results, Promise};

use crate::api::Session;

pub use crate::schema::auth_capnp;
use crate::db::Databases;
use crate::db::pass::PassDB;
use crate::db::user::{Internal as UserDB};

pub struct AppData {
    passdb: Arc<PassDB>,
    userdb: Arc<UserDB>,
}
pub struct SessionData {
    session: Arc<Session>,
}


struct CB;
impl Callback<AppData, SessionData> for CB {
    fn callback(mut sasl: SaslCtx<AppData, SessionData>, mut session: SaslSession<SessionData>, prop: Property) -> libc::c_int {
        let ret = match prop {
            Property::GSASL_VALIDATE_SIMPLE => {
                // FIXME: get_property and retrive_mut can't be used interleaved but that's
                // technically safe.
                let cap_session = session.retrieve_mut().map(|sd| sd.session.clone());

                let authid = match session.get_property(Property::GSASL_AUTHID) {
                    None => return ReturnCode::GSASL_NO_AUTHID as libc::c_int,
                    Some(a) => {
                        match a.to_str() {
                            Ok(s) => s,
                            Err(_) => return ReturnCode::GSASL_SASLPREP_ERROR as libc::c_int,
                        }
                    },
                };

                let pass = session.get_property(Property::GSASL_PASSWORD);
                if pass.is_none() {
                    return ReturnCode::GSASL_NO_PASSWORD as libc::c_int;
                }
                let pass = pass.unwrap();


                if let Some(appdata) = sasl.retrieve_mut() {
                    if let Ok(Some(b)) = appdata.passdb.check(authid, pass.to_bytes()) {
                        if b {
                            if let Some(s) = cap_session {
                                if let Ok(Some(user)) = appdata.userdb.get_user(authid) {
                                    s.user.try_lock().unwrap().replace(user);
                                }
                            }

                            ReturnCode::GSASL_OK
                        } else {
                            ReturnCode::GSASL_AUTHENTICATION_ERROR
                        }
                    } else {
                        ReturnCode::GSASL_AUTHENTICATION_ERROR
                    }
                } else {
                    ReturnCode::GSASL_AUTHENTICATION_ERROR
                }
            }
            p => {
                println!("Callback called with property {:?}", p);
                ReturnCode::GSASL_NO_CALLBACK 
            }
        };
        ret as libc::c_int
    }
}

pub struct Auth {
    pub ctx: SASL<AppData, SessionData>,
    session: Arc<Session>,
}

impl Auth {
    pub fn new(dbs: Databases, session: Arc<Session>) -> Self {
        let mut ctx = SASL::new().unwrap();

        let appdata = Box::new(AppData { passdb: dbs.passdb.clone(), userdb: dbs.userdb.clone() });

        ctx.store(appdata);

        ctx.install_callback::<CB>();

        info!(session.log, "Auth created");

        Self { ctx, session }
    }
}

use auth_capnp::authentication::*;
impl auth_capnp::authentication::Server for Auth {
    fn mechanisms(&mut self, 
        _: Params<mechanisms_params::Owned>,
        mut res: Results<mechanisms_results::Owned>
    ) -> Promise<(), capnp::Error> {
        let mechs = match self.ctx.server_mech_list() {
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
        }

        Promise::ok(())
    }

    // TODO: return Outcome instead of exceptions
    fn start(&mut self,
        params: Params<start_params::Owned>,
        mut res: Results<start_results::Owned>
    ) -> Promise<(), capnp::Error> {
        let req = pry!(pry!(params.get()).get_request());

        // Extract the MECHANISM the client wants to use and start a session.
        // Or fail at that and thrown an exception TODO: return Outcome
        let mech = pry!(req.get_mechanism());
        let mut session = match self.ctx.server_start(mech) {
            Ok(s) => s,
            Err(e) => 
                return Promise::err(capnp::Error {
                    kind: capnp::ErrorKind::Failed,
                    description: format!("SASL error: {}", e),
                }),
        };

        session.store(Box::new(SessionData { session: self.session.clone() }));

        // If the client has provided initial data go use that
        use auth_capnp::request::initial_response::Which;
        let step_res = match req.get_initial_response().which() {
            Err(capnp::NotInSchema(_)) => 
                return Promise::err(capnp::Error {
                    kind: capnp::ErrorKind::Failed,
                    description: "Initial data is badly formatted".to_string(),
                }),

            Ok(Which::None(_)) => {
                // FIXME: Actually this needs to indicate NO data instead of SOME data of 0 length
                session.step(&[])
            }
            Ok(Which::Initial(data)) => {
                session.step(pry!(data))
            }
        };

        // The step may either return an error, a success or the need for more data
        // TODO: Set the session user. Needs a lookup though <.>
        match step_res {
            Ok(Step::Done(b)) => {
                use auth_capnp::response::Result;

                let mut outcome = pry!(res.get().get_response()).init_outcome();
                outcome.reborrow().set_result(Result::Successful);
                if b.len() != 0 {
                    outcome.init_additional_data().set_additional(&b);
                }
                Promise::ok(())
            },
            Ok(Step::NeedsMore(b)) => {
                pry!(res.get().get_response()).set_challence(&b);
                Promise::ok(())
            }
            // TODO: This should really be an outcome because this is failed auth just as much atm.
            Err(e) => 
                return Promise::err(capnp::Error {
                    kind: capnp::ErrorKind::Failed,
                    description: format!("SASL error: {}", e),
                }),
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
