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
use crate::machine::Machine as NwMachine;

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
        // TODO check permissions
        builder.set_read(capnp_rpc::new_client(Read(self.clone())));
        // TODO set all the others
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

struct Read(Arc<Machine>);

impl read::Server for Read {
    fn info(&mut self,
        _params: read::InfoParams,
        mut results: read::InfoResults) 
    -> Promise<(), Error>
    {
        let mut b = results.get().init_minfo();
        self.0.fill_info(&mut b);
        Promise::ok(())
    }
}

struct Write(Arc<Machine>);

impl write::Server for Write {
    fn use_(&mut self,
        _params: write::UseParams,
        results: write::UseResults)
    -> Promise<(), Error>
    {
        let uid = self.0.session.user.as_ref().map(|u| u.id.clone());
        let new_state = MachineState::used(uid.clone());
        let this = self.0.clone();
        let f = this.machine.request_state_change(this.session.user.as_ref(), new_state)
            .map(|res_token| match res_token {
                Ok(tok) => {
                    return Ok(());
                },
                Err(e) => Err(capnp::Error::failed("State change request returned an err".to_string())),
        });

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
