use std::sync::Arc;

use slog::Logger;

use smol::net::TcpStream;

use crate::error::{Error, Result};
use crate::api::auth;
use crate::api::Bootstrap;

use capnp_rpc::{twoparty, rpc_twoparty_capnp};

use crate::schema::connection_capnp;

/// Connection context
// TODO this should track over several connections
pub struct Session {
    pub log: Logger,
    pub user: Option<auth::User>,
}

impl Session {
    pub fn new(log: Logger) -> Self {
        let user = None;

        Session { log, user }
    }
}

async fn handshake(log: &Logger, stream: &mut TcpStream) -> Result<()> {
    if let Some(m) = capnp_futures::serialize::read_message(stream.clone(), Default::default()).await? {
        let greeting = m.get_root::<connection_capnp::greeting::Reader>()?;
        let major = greeting.get_major();
        let minor = greeting.get_minor();

        if major != 0 {
            Err(Error::BadVersion((major, minor)))
        } else {
            let program = format!("{}-{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

            let mut answer = ::capnp::message::Builder::new_default();
            let mut b = answer.init_root::<connection_capnp::greeting::Builder>();
            b.set_program(&program);
            b.set_host("localhost");
            b.set_major(0);
            b.set_minor(1);
            capnp_futures::serialize::write_message(stream, answer).await?;
            info!(log, "Handshake successful with peer {} running {}, API {}.{}", 
                greeting.get_host()?, greeting.get_program()?, major, minor);
            Ok(())
        }
    } else {
        unimplemented!()
    }
}

pub async fn handle_connection(log: Logger, stream: TcpStream) -> Result<()> {
    //handshake(&log, &mut stream).await?;

    info!(log, "New connection from on {:?}", stream);
    let session = Arc::new(Session::new(log));
    let boots = Bootstrap::new(session);
    let rpc: connection_capnp::bootstrap::Client = capnp_rpc::new_client(boots);

    let network = twoparty::VatNetwork::new(stream.clone(), stream,
        rpc_twoparty_capnp::Side::Server, Default::default());
    let rpc_system = capnp_rpc::RpcSystem::new(Box::new(network), 
        Some(rpc.client));

    rpc_system.await.unwrap();
    Ok(())
}
