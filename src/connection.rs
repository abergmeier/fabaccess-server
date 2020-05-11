use slog::Logger;

use async_std::net::TcpStream;
use futures::io::AsyncWriteExt;

use crate::error::Result;

pub mod gen {
    include!(concat!(env!("OUT_DIR"), "/schema/connection_capnp.rs"));
}


pub async fn handle_connection(log: Logger, mut stream: TcpStream) -> Result<()> {
    let host = "localhost";
    let program = "Difluoroborane-0.1.0";
    let version = (0u32,1u32);

    let mut message = capnp::message::Builder::new_default();
    let greet_outer = message.init_root::<gen::message::Builder>();
    let mut greeting = greet_outer.init_greet();
    greeting.set_host(host);
    greeting.set_program(program);
    greeting.set_major(version.0);
    greeting.set_minor(version.1);

    capnp_futures::serialize::write_message(&mut stream, message).await?;

    stream.flush().await?;

    let receive_options = capnp::message::ReaderOptions::default();
    let message = capnp_futures::serialize::read_message(&mut stream, receive_options).await.unwrap().unwrap();
    let body: capnp::any_pointer::Reader = message.get_root().unwrap();
    let m = body.get_as::<gen::message::Reader>().unwrap();

    if m.has_greet() {
        match m.which() {
            Ok(gen::message::Which::Greet(Ok(r))) => {
                println!("Host {} with program {} is saying hello. They speak API version {}.{}.",
                    r.get_host().unwrap(),
                    r.get_program().unwrap(),
                    r.get_major(),
                    r.get_minor())
            },
            _ => {
                // We *JUST* checked that it's a greeting. This can not happen
                unreachable!()
            }
        }
    }

    Ok(())
}
