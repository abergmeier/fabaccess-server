//! Authentication subsystem
//!
//! Authorization is over in `access.rs`
//! Authentication using SASL

use slog::Logger;

use rsasl::{SASL, Property, Session, ReturnCode};
use rsasl::sys::{Gsasl, Gsasl_session};

use crate::error::Result;
use crate::config::Settings;

pub use crate::schema::auth_capnp;

extern "C" fn callback(ctx: *mut Gsasl, sctx: *mut Gsasl_session, prop: Property) -> i32 {
    let sasl = SASL::from_ptr(ctx);
    let mut session = Session::from_ptr(sctx);

    let rc = match prop {
        Property::GSASL_VALIDATE_SIMPLE => {
            let authid = session.get_property_fast(Property::GSASL_AUTHID).to_string_lossy();
            let pass = session.get_property_fast(Property::GSASL_PASSWORD).to_string_lossy();

            if authid == "test" && pass == "secret" {
                ReturnCode::GSASL_OK
            } else {
                ReturnCode::GSASL_AUTHENTICATION_ERROR
            }
        }
        p => {
            println!("Callback called with property {:?}", p);
            ReturnCode::GSASL_NO_CALLBACK 
        }
    };

    rc as i32
}

pub struct Auth {
    pub ctx: SASL,
}

impl Auth {
    pub fn new() -> Self {
        let mut ctx = SASL::new().unwrap();

        ctx.install_callback(Some(callback));

        Self { ctx }
    }
}

pub async fn init(log: Logger, config: Settings) -> Result<Auth> {
    Ok(Auth::new())
}

// Use the newtype pattern here to make the type system work for us; even though AuthCId is for all
// intents and purposes just a String the compiler will still complain if you return or more
// importantly pass a String intead of a AuthCId. This prevents bugs where you get an object from
// somewhere and pass it somewhere else and in between don't check if it's the right type and
// accidentally pass the authzid where the authcid should have gone.

/// Authentication Identity
///
/// Under the hood a string because the form depends heavily on the method
struct AuthCId(String);
/// Authorization Identity
///
/// This identity is internal to FabAccess and completely independent from the authentication
/// method or source
struct AuthZId {
    uid: String,
    subuid: String,
    domain: String,
}

// What is a man?! A miserable little pile of secrets!
/// Authentication/Authorization user object.
///
/// This struct contains the user as is passed to the actual authentication/authorization
/// subsystems
///
struct User {
    /// Contains the Authentication ID used
    ///
    /// The authentication ID is an identifier for the authentication exchange. This is different
    /// than the ID of the user to be authenticated; for example when using x509 the authcid is
    /// the dn of the certificate, when using GSSAPI the authcid is of form `<userid>@<REALM>`
    authcid: AuthCId,

    /// Contains the Authorization ID
    ///
    /// This is the identifier of the user to *authenticate as*. This in several cases is different
    /// to the `authcid`: 
    /// If somebody wants to authenticate as somebody else, su-style.
    /// If a person wants to authenticate as a higher-permissions account, e.g. foo may set authzid foo+admin
    /// to split normal user and "admin" accounts.
    /// If a method requires a specific authcid that is different from the identifier of the user
    /// to authenticate as, e.g. GSSAPI, x509 client certificates, API TOKEN authentication.
    authzid: AuthZId,

    /// Contains the authentication method used
    ///
    /// For the most part this is the SASL method
    authMethod: String,

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

enum AuthError {
    /// Authentication ID is bad/unknown/..
    BadAuthcid,
    /// Authorization ID is bad/unknown/..
    BadAuthzid,
    /// User may not use that authorization id
    NotAllowedAuthzid,

}

fn grant_auth(user: User) -> std::result::Result<(), AuthError> {
    unimplemented!()
}
