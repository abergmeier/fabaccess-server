mod server;
pub use server::FabFire;

use rsasl::mechname::Mechname;
use rsasl::registry::{Matches, Mechanism, Named, Side, MECHANISMS};

const MECHNAME: &'static Mechname = &Mechname::const_new_unchecked(b"X-FABFIRE-BIN");

#[linkme::distributed_slice(MECHANISMS)]
pub static FABFIRE: Mechanism = Mechanism::build(
    MECHNAME,
    300,
    None,
    Some(FabFire::new_server),
    Side::Client,
    |_| Some(Matches::<Select>::name()),
    |_| true,
);

struct Select;
impl Named for Select {
    fn mech() -> &'static Mechanism {
        &FABFIRE
    }
}
