pub use capnpc::schema_capnp;

pub mod authenticationsystem_capnp {
    include!(concat!(env!("OUT_DIR"), "/authenticationsystem_capnp.rs"));
}

pub mod connection_capnp {
    include!(concat!(env!("OUT_DIR"), "/connection_capnp.rs"));
}

pub mod general_capnp {
    include!(concat!(env!("OUT_DIR"), "/general_capnp.rs"));
}

pub mod machine_capnp {
    include!(concat!(env!("OUT_DIR"), "/machine_capnp.rs"));
}

pub mod machinesystem_capnp {
    include!(concat!(env!("OUT_DIR"), "/machinesystem_capnp.rs"));
}

pub mod permissionsystem_capnp {
    include!(concat!(env!("OUT_DIR"), "/permissionsystem_capnp.rs"));
}

pub mod role_capnp {
    include!(concat!(env!("OUT_DIR"), "/role_capnp.rs"));
}

pub mod space_capnp {
    include!(concat!(env!("OUT_DIR"), "/space_capnp.rs"));
}

pub mod user_capnp {
    include!(concat!(env!("OUT_DIR"), "/user_capnp.rs"));
}

pub mod usersystem_capnp {
    include!(concat!(env!("OUT_DIR"), "/usersystem_capnp.rs"));
}
