//! Authentication subsystem
//!
//! Authorization is over in `access.rs`
//! Authentication using SASL

use slog::Logger;

use rsasl::{SASL, Property, Step, Session, ReturnCode};
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
        _ => { ReturnCode::GSASL_NO_CALLBACK }
    };

    rc as i32
}

pub struct Auth {
    ctx: SASL,
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
