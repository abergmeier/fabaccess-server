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

use std::marker::PhantomData;
use rsasl::property::{Property, PropertyQ, PropertyDefinition};
// All Property types must implement Debug.
#[derive(Debug)]
// The `PhantomData` in the constructor is only used so external crates can't construct this type.
pub struct FabFireCardKey(PhantomData<()>);
impl PropertyQ for FabFireCardKey {
    // This is the type stored for this property. This could also be the struct itself if you
    // so choose
    type Item = [u8; 16];
    // You need to return the constant you define below here for things to work properly
    fn property() -> Property {
        FABFIRECARDKEY
    }
}
// This const is used by your mechanism to query and by your users to set your property. It
// thus needs to be exported from your crate
pub const FABFIRECARDKEY: Property = Property::new(&PropertyDefinition::new(
    // Short name, used in `Debug` output
    "FabFireCardKey",
    // A longer user-facing name used in `Display` output
    "A AES128 key for a FabFire card",
));
