pub use capnpc::schema_capnp;

#[cfg(feature = "generated")]
pub mod auth_capnp {
    include!(concat!(env!("OUT_DIR"), "/auth_capnp.rs"));
}
#[cfg(not(feature = "generated"))]
pub mod auth_capnp;

#[cfg(feature = "generated")]
pub mod main_capnp {
    include!(concat!(env!("OUT_DIR"), "/main_capnp.rs"));
}
#[cfg(not(feature = "generated"))]
pub mod main_capnp;

#[cfg(feature = "generated")]
pub mod utils_capnp {
    include!(concat!(env!("OUT_DIR"), "/utils_capnp.rs"));
}
#[cfg(not(feature = "generated"))]
pub mod utils_capnp;

#[cfg(feature = "generated")]
pub mod resource_capnp {
    include!(concat!(env!("OUT_DIR"), "/resource_capnp.rs"));
}
#[cfg(not(feature = "generated"))]
pub mod resource_capnp;

#[cfg(feature = "generated")]
pub mod resources_capnp {
    include!(concat!(env!("OUT_DIR"), "/resources_capnp.rs"));
}
#[cfg(not(feature = "generated"))]
pub mod resources_capnp;

#[cfg(feature = "generated")]
pub mod role_capnp {
    include!(concat!(env!("OUT_DIR"), "/role_capnp.rs"));
}
#[cfg(not(feature = "generated"))]
pub mod role_capnp;

#[cfg(feature = "generated")]
pub mod user_capnp {
    include!(concat!(env!("OUT_DIR"), "/user_capnp.rs"));
}
#[cfg(not(feature = "generated"))]
pub mod user_capnp;

#[cfg(feature = "generated")]
pub mod users_capnp {
    include!(concat!(env!("OUT_DIR"), "/users_capnp.rs"));
}
#[cfg(not(feature = "generated"))]
pub mod users_capnp;