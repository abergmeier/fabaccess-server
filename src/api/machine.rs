use std::sync::Arc;
use std::ops::Deref;

use capnp::capability::Promise;
use capnp::Error;

use futures::FutureExt;

use crate::schema::api_capnp::State;
use crate::schema::api_capnp::machine::*;
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
        builder.set_read(capnp_rpc::new_client(Read(self.clone())));
        builder.set_write(capnp_rpc::new_client(Write(self.clone())));
        builder.set_manage(capnp_rpc::new_client(Manage(self.clone())));
        builder.set_admin(capnp_rpc::new_client(Admin(self.clone())));
    }

    pub async fn fill_info(&self, builder: &mut m_info::Builder<'_>) {
        let guard = self.machine.lock().await;

        builder.set_name(guard.desc.name.as_ref());

        if let Some(desc) = guard.desc.description.as_ref() {
            builder.set_description(desc);
        }

        match guard.read_state().lock_ref().deref().state {
            Status::Free => {
                builder.set_state(State::Free);
            }
            Status::Disabled => {
                builder.set_state(State::Disabled);
            }
            Status::Blocked(_) => {
                builder.set_state(State::Blocked);
            }
            Status::InUse(_) => {
                builder.set_state(State::InUse);
            }
            Status::ToCheck(_) => {
                builder.set_state(State::ToCheck);
            }
            Status::Reserved(_) => {
                builder.set_state(State::Reserved);
            }
        }
    }
}

#[derive(Clone)]
pub struct Read(Arc<Machine>);

impl Read {
    pub fn new(inner: Arc<Machine>) -> Self {
        Self(inner)
    }
}

impl read::Server for Read {
    fn info(&mut self,
        _params: read::InfoParams,
        mut results: read::InfoResults) 
    -> Promise<(), Error>
    {
        let this = self.clone();
        let f = async move {
            let mut b = results.get().init_minfo();

            this.0.fill_info(&mut b).await;

            Ok(())
        };

        Promise::from_future(f)
    }
}

struct Write(Arc<Machine>);

impl write::Server for Write {
    fn use_(&mut self,
        _params: write::UseParams,
        mut results: write::UseResults)
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
                    let gb = GiveBack(Some(tok));
                    results.get().set_ret(capnp_rpc::new_client(gb));

                    return Ok(());
                },
                Err(e) => Err(capnp::Error::failed(format!("State change request returned {}", e))),
            }
        };

        Promise::from_future(f)
    }

    fn reserve(&mut self,
        _params: write::ReserveParams,
        _results: write::ReserveResults)
    -> Promise<(), Error>
    {
        unimplemented!()
    }
}

struct GiveBack(Option<ReturnToken>);

impl write::give_back::Server for GiveBack {
    fn ret(&mut self,
        _params: write::give_back::RetParams,
        _results: write::give_back::RetResults)
    -> Promise<(), Error>
    {
        if let Some(chan) = self.0.take() {
            chan.send(())
                .expect("Other end of GiveBack token was dropped?!");
        }

        Promise::ok(())
    }
}

struct Manage(Arc<Machine>);

impl manage::Server for Manage {
    fn ok(&mut self,
        _params: manage::OkParams,
        _results: manage::OkResults)
    -> Promise<(), Error>
    {
        unimplemented!()
    }
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
