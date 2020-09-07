//! Authentication subsystem
//!
//! Authorization is over in `access.rs`
//! Authentication using SASL

use slog::Logger;

use rsasl::{SASL, Property, Session, ReturnCode};
use rsasl::sys::{Gsasl, Gsasl_session};

use crate::error::Result;
use crate::config::Config;

pub mod gen {
    include!(concat!(env!("OUT_DIR"), "/schema/auth_capnp.rs"));
}

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

pub async fn init(log: Logger, config: Config) -> Result<Auth> {
    Ok(Auth::new())
}
