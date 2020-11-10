#[allow(dead_code)]
pub mod auth_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/auth_capnp.rs"));
}

#[allow(dead_code)]
pub mod api_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/api_capnp.rs"));
}

#[allow(dead_code)]
pub mod connection_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/connection_capnp.rs"));
}
