pub mod auth_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/auth_capnp.rs"));
}

pub mod api_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/api_capnp.rs"));
}

pub mod connection_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/connection_capnp.rs"));
}
