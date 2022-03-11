mod server;
pub use server::FabFire;

use rsasl::mechname::Mechname;
use rsasl::registry::{Mechanism, MECHANISMS};
use rsasl::session::Side;

const MECHNAME: &'static Mechname = &Mechname::const_new_unchecked(b"X-FABFIRE");

#[linkme::distributed_slice(MECHANISMS)]
pub static FABFIRE: Mechanism = Mechanism {
    mechanism: MECHNAME,
    priority: 300,
    // In this situation there's one struct for both sides, however you can just as well use
    // different types than then have different `impl Authentication` instead of checking a value
    // in self.
    client: None,
    server: Some(FabFire::new_server),
    first: Side::Client,
};
