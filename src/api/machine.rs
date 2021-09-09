use std::sync::Arc;
use std::ops::Deref;

use capnp::capability::Promise;
use capnp::Error;

use futures::FutureExt;

use crate::schema::machine_capnp::machine::*;
use crate::connection::Session;
use crate::db::Databases;
use crate::db::machine::{Status, MachineState};
use crate::machine::{Machine as NwMachine, ReturnToken};

#[derive(Clone)]
pub struct Machine {
    session: Arc<Session>,
    machine: NwMachine,
    db: Databases,
}

impl Machine {
    pub fn new(session: Arc<Session>, machine: NwMachine, db: Databases) -> Self {
        Machine { session, machine, db }
    }

    pub fn fill(self: Arc<Self>, builder: &mut Builder) {
        builder.set_manage(capnp_rpc::new_client(Manage(self.clone())));
        builder.set_admin(capnp_rpc::new_client(Admin(self.clone())));
    }
}

#[derive(Clone)]
pub struct Read(Arc<Machine>);

impl Read {
    pub fn new(inner: Arc<Machine>) -> Self {
        Self(inner)
    }
}

impl info::Server for Read {
}

struct Write(Arc<Machine>);

impl use_::Server for Write {
    fn use_(&mut self,
        _params: use_::UseParams,
        mut results: use_::UseResults)
    -> Promise<(), Error>
    {
        let uid = self.0.session.user.try_lock().unwrap().as_ref().map(|u| u.id.clone());
        let new_state = MachineState::used(uid.clone());
        let this = self.0.clone();
        let f = async move {
            let res_token = this.machine.request_state_change(
                this.session.user.try_lock().unwrap().as_ref(), 
                new_state
            ).await;

            match res_token {
                // TODO: Do something with the token we get returned
                Ok(tok) => {
                    return Ok(());
                },
                Err(e) => Err(capnp::Error::failed(format!("State change request returned {}", e))),
            }
        };

        Promise::from_future(f)
    }

    fn reserve(&mut self,
        _params: use_::ReserveParams,
        _results: use_::ReserveResults)
    -> Promise<(), Error>
    {
        unimplemented!()
    }

}

impl in_use::Server for Write {
    fn give_back(&mut self,
        _params: in_use::GiveBackParams,
        mut results: in_use::GiveBackResults)
    -> Promise<(), Error>
    {
        let this = self.0.clone();

        let f = async move {
            let status = this.machine.get_status().await;
            let sess = this.session.clone();

            match status {
                Status::InUse(Some(uid)) => {
                    let user = sess.user.lock().await;
                    if let Some(u) = user.as_ref() {
                        if u.id == uid {
                        }
                    }
                },
                // Machine not in use
                _ => {
                }
            }
        };

        Promise::from_future(f.map(|_| Ok(())))
    }
}

struct Manage(Arc<Machine>);

impl manage::Server for Manage {
}

struct Admin(Arc<Machine>);

impl admin::Server for Admin {
    fn force_set_state(&mut self,
        _params: admin::ForceSetStateParams,
        _results: admin::ForceSetStateResults)
    -> Promise<(), Error>
    {
        unimplemented!()
    }

    fn force_set_user(&mut self,
        _params: admin::ForceSetUserParams,
        _results: admin::ForceSetUserResults)
    -> Promise<(), Error>
    {
        unimplemented!()
    }
}
