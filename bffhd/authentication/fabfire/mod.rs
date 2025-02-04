mod server;
pub use server::FabFire;

use rsasl::mechname::Mechname;
use rsasl::registry::{Matches, Mechanism, Named, Side, MECHANISMS};

const MECHNAME: &'static Mechname = &Mechname::const_new_unchecked(b"X-FABFIRE");

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

use rsasl::property::SizedProperty;
use std::marker::PhantomData;

// All Property types must implement Debug.
#[derive(Debug)]
// The `PhantomData` in the constructor is only used so external crates can't construct this type.
pub struct FabFireCardKey(PhantomData<()>);

impl SizedProperty<'_> for FabFireCardKey {
    type Value = [u8; 16];
    const DESCRIPTION: &'static str = "A AES128 key for a FabFire card";
}
