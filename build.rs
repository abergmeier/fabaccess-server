fn main() {
    ::capnpc::CompilerCommand::new().file("schema/connection.capnp").run().unwrap();
    ::capnpc::CompilerCommand::new().file("schema/api.capnp").run().unwrap();
    ::capnpc::CompilerCommand::new().file("schema/auth.capnp").run().unwrap();
}
