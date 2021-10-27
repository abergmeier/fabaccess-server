pub use capnpc::schema_capnp;

#[allow(dead_code)]
pub mod auth_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/auth_capnp.rs"));
}

#[allow(dead_code)]
pub mod main_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/main_capnp.rs"));
}

#[allow(dead_code)]
pub mod utils_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/utils_capnp.rs"));
}

#[allow(dead_code)]
pub mod resource_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/resource_capnp.rs"));
}

#[allow(dead_code)]
pub mod resources_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/resources_capnp.rs"));
}

#[allow(dead_code)]
pub mod role_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/role_capnp.rs"));
}

#[allow(dead_code)]
pub mod user_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/user_capnp.rs"));
}

#[allow(dead_code)]
pub mod users_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/users_capnp.rs"));
}
