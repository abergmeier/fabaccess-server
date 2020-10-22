use slog::Logger;

use smol::net::TcpStream;

use crate::error::Result;
use crate::auth;
use crate::api;

pub use crate::schema::connection_capnp;

pub async fn handle_connection(log: Logger, mut stream: TcpStream) -> Result<()> {
    let host = "localhost";
    let program = "Difluoroborane-0.1.0";
    let version = (0u32,1u32);


    let receive_options = capnp::message::ReaderOptions::default();
    {
        let message = capnp_futures::serialize::read_message(&mut stream, receive_options).await.unwrap().unwrap();
        let m = message.get_root::<connection_capnp::message::Reader>().unwrap();

        if m.has_greet() {
            match m.which() {
                Ok(connection_capnp::message::Which::Greet(Ok(r))) => {
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
    }

    {
        let mut message = capnp::message::Builder::new_default();
        let greet_outer = message.init_root::<connection_capnp::message::Builder>();
        let mut greeting = greet_outer.init_greet();
        greeting.set_host(host);
        greeting.set_program(program);
        greeting.set_major(version.0);
        greeting.set_minor(version.1);

        capnp_futures::serialize::write_message(&mut stream, message).await?;
    }
    {
        let mut message = capnp::message::Builder::new_default();
        let outer = message.init_root::<connection_capnp::message::Builder>();
        let mut mechs = outer.init_auth().init_mechanisms(1);
        mechs.set(0, "PLAIN");

        capnp_futures::serialize::write_message(&mut stream, message).await?;
    }

    {
        let message = capnp_futures::serialize::read_message(&mut stream, receive_options).await.unwrap().unwrap();
        let m = message.get_root::<connection_capnp::message::Reader>().unwrap();

        let mut auth_success = false;

        match m.which() {
            Ok(connection_capnp::message::Which::Auth(Ok(r))) => {
                if let Ok(w) = r.which() {
                    use crate::auth::auth_capnp::auth_message::*;
                    match w {
                        Request(Ok(r)) => {
                            let m = r.get_mechanism().unwrap();
                            println!("Client wants to AUTH using {:?}",
                                m);
                            let cm = std::ffi::CString::new(m).unwrap();
                            let mut sasl = auth::Auth::new();
                            let mut sess = sasl.ctx.server_start(&cm).unwrap();

                            use crate::auth::auth_capnp::request::initial_response::*;
                            match r.get_initial_response().which() {
                                Ok(Initial(Ok(r))) => {
                                    debug!(log, "Client Auth with initial data");
                                    let mut message = capnp::message::Builder::new_default();
                                    let mut outer = message.init_root::<connection_capnp::message::Builder>().init_auth();

                                    match sess.step(r) {
                                        Ok(rsasl::Step::Done(b)) => {
                                            auth_success = true;
                                            debug!(log, "Authentication successful");
                                            let mut outcome= outer.init_outcome();

                                            outcome.set_result(auth::auth_capnp::outcome::Result::Successful);
                                            if !b.is_empty() {
                                                let mut add_data = outcome.init_additional_data();
                                                add_data.set_additional(&b);
                                            }
                                        },
                                        Ok(rsasl::Step::NeedsMore(b)) => {
                                            debug!(log, "Authentication needs more data");
                                            outer.set_response(&b);
                                        }
                                        Err(e) => {
                                            warn!(log, "Authentication error: {}", e);
                                            let mut outcome = outer.init_outcome();

                                            // TODO: Distinguish errors
                                            outcome.set_result(auth::auth_capnp::outcome::Result::Failed);
                                            outcome.set_action(auth::auth_capnp::outcome::Action::Retry);
                                            outcome.set_help_text(&format!("{}", e));
                                        }
                                    }

                                    capnp_futures::serialize::write_message(&mut stream, message).await?;
                                }
                                _ => {
                                }
                            }
                        },
                        _ => {
                        }
                    }
                } else {
                    println!("Got unexpected message");
                }
            },
            Ok(_) => {
                println!("Got unexpected message");
            }
            Err(e) => {
                println!("Got error {:?}", e);
            }
        }

        if auth_success {
            info!(log, "Handing off to API connection handler");
            api::handle_connection(log, stream).await;
        }
    }

    Ok(())
}
