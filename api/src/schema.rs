pub use capnpc::schema_capnp;

#[allow(dead_code)]
pub mod auth_capnp {
    include!(concat!(env!("OUT_DIR"), "/auth_capnp.rs"));
}

#[allow(dead_code)]
pub mod main_capnp {
    include!(concat!(env!("OUT_DIR"), "/main_capnp.rs"));
}

#[allow(dead_code)]
pub mod utils_capnp {
    include!(concat!(env!("OUT_DIR"), "/utils_capnp.rs"));
}

#[allow(dead_code)]
pub mod resource_capnp {
    include!(concat!(env!("OUT_DIR"), "/resource_capnp.rs"));
}

#[allow(dead_code)]
pub mod resources_capnp {
    include!(concat!(env!("OUT_DIR"), "/resources_capnp.rs"));
}

#[allow(dead_code)]
pub mod role_capnp {
    include!(concat!(env!("OUT_DIR"), "/role_capnp.rs"));
}

#[allow(dead_code)]
pub mod user_capnp {
    include!(concat!(env!("OUT_DIR"), "/user_capnp.rs"));
}

#[allow(dead_code)]
pub mod users_capnp {
    include!(concat!(env!("OUT_DIR"), "/users_capnp.rs"));
}