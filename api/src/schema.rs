pub use capnpc::schema_capnp;


#[cfg(feature = "generated")]
pub mod authenticationsystem_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/authenticationsystem_capnp.rs"));
}
#[cfg(not(feature = "generated"))]
pub mod authenticationsystem_capnp;

#[cfg(feature = "generated")]
pub mod connection_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/connection_capnp.rs"));
}
#[cfg(not(feature = "generated"))]
pub mod connection_capnp;

#[cfg(feature = "generated")]
pub mod general_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/general_capnp.rs"));
}
#[cfg(not(feature = "generated"))]
pub mod general_capnp;

#[cfg(feature = "generated")]
pub mod machine_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/machine_capnp.rs"));
}
#[cfg(not(feature = "generated"))]
pub mod machine_capnp;

#[cfg(feature = "generated")]
pub mod machinesystem_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/machinesystem_capnp.rs"));
}
#[cfg(not(feature = "generated"))]
pub mod machinesystem_capnp;

#[cfg(feature = "generated")]
pub mod permissionsystem_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/permissionsystem_capnp.rs"));
}
#[cfg(not(feature = "generated"))]
pub mod permissionsystem_capnp;

#[cfg(feature = "generated")]
pub mod role_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/role_capnp.rs"));
}
#[cfg(not(feature = "generated"))]
pub mod role_capnp;

#[cfg(feature = "generated")]
pub mod space_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/space_capnp.rs"));
}
#[cfg(not(feature = "generated"))]
pub mod space_capnp;

#[cfg(feature = "generated")]
pub mod user_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/user_capnp.rs"));
}
#[cfg(not(feature = "generated"))]
pub mod user_capnp;

#[cfg(feature = "generated")]
pub mod usersystem_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/usersystem_capnp.rs"));
}
#[cfg(not(feature = "generated"))]
pub mod usersystem_capnp;
