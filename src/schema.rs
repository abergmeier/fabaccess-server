#[allow(dead_code)]
pub mod authenticationsystem_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/authenticationsystem_capnp.rs"));
}

#[allow(dead_code)]
pub mod connection_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/connection_capnp.rs"));
}

#[allow(dead_code)]
pub mod general_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/general_capnp.rs"));
}

#[allow(dead_code)]
pub mod machine_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/machine_capnp.rs"));
}

#[allow(dead_code)]
pub mod machinesystem_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/machinesystem_capnp.rs"));
}

#[allow(dead_code)]
pub mod permissionsystem_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/permissionsystem_capnp.rs"));
}

#[allow(dead_code)]
pub mod role_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/role_capnp.rs"));
}

#[allow(dead_code)]
pub mod space_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/space_capnp.rs"));
}

#[allow(dead_code)]
pub mod user_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/user_capnp.rs"));
}

#[allow(dead_code)]
pub mod usersystem_capnp {
    include!(concat!(env!("OUT_DIR"), "/schema/usersystem_capnp.rs"));
}
