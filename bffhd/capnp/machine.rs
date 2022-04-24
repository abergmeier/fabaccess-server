use crate::resources::modules::fabaccess::{ArchivedStatus, Status};
use crate::resources::Resource;
use crate::session::SessionHandle;
use api::machine_capnp::machine::{
    self,
    admin, admin::Server as AdminServer, check, check::Server as CheckServer, in_use as inuse,
    in_use::Server as InUseServer, info, info::Server as InfoServer, manage,
    manage::Server as ManageServer, use_, use_::Server as UseServer,

    MachineState,
};
use api::general_capnp::optional;
use capnp::capability::Promise;
use capnp_rpc::pry;
use crate::capnp::user::User;

#[derive(Clone)]
pub struct Machine {
    session: SessionHandle,
    resource: Resource,
}

impl Machine {
    pub fn new(session: SessionHandle, resource: Resource) -> Self {
        Self { session, resource }
    }

    pub fn build_into(self, mut builder: machine::Builder) {
        builder.set_id(self.resource.get_id());
        builder.set_name(self.resource.get_name());
        if let Some(ref desc) = self.resource.get_description().description {
            builder.set_description(desc);
        }
        if let Some(ref wiki) = self.resource.get_description().wiki {
            builder.set_wiki(wiki);
        }
        if let Some(ref category) = self.resource.get_description().category {
            builder.set_category(category);
        }
        builder.set_urn(&format!("urn:fabaccess:resource:{}", self.resource.get_id()));

        {
            let user = self.session.get_user_ref();
            let state = self.resource.get_state_ref();
            let state = state.as_ref();

            if self.session.has_write(&self.resource) && match &state.inner.state {
                ArchivedStatus::Free => true,
                ArchivedStatus::Reserved(reserver) if reserver == &user => true,
                _ => false,
            } {
                builder.set_use(capnp_rpc::new_client(self.clone()));
            }

            if self.session.has_manage(&self.resource) {
                builder.set_manage(capnp_rpc::new_client(self.clone()));
            }

            // TODO: admin perm

            let s = match &state.inner.state {
                ArchivedStatus::Free => MachineState::Free,
                ArchivedStatus::Disabled => MachineState::Disabled,
                ArchivedStatus::Blocked(_) => MachineState::Blocked,
                ArchivedStatus::InUse(owner) => {
                    if owner == &user {
                        builder.set_inuse(capnp_rpc::new_client(self.clone()));
                    }
                    MachineState::InUse
                },
                ArchivedStatus::Reserved(_) => MachineState::Reserved,
                ArchivedStatus::ToCheck(_) => MachineState::ToCheck,
            };
            if self.session.has_read(&self.resource) {
                builder.set_state(s);
            }
        }

        builder.set_info(capnp_rpc::new_client(self));
    }

    /// Builds a machine into the given builder. Re
    pub fn build(session: SessionHandle, resource: Resource, builder: machine::Builder) {
        let this = Self::new(session.clone(), resource.clone());
        this.build_into(builder)
    }

    pub fn optional_build(session: SessionHandle, resource: Resource, builder: optional::Builder<machine::Owned>) {
        let this = Self::new(session.clone(), resource.clone());
        if this.resource.visible(&session) || session.has_read(&resource) {
            let mut builder = builder.init_just();
            this.build_into(builder);
        }
    }
}

impl InfoServer for Machine {
    fn get_property_list(
        &mut self,
        _: info::GetPropertyListParams,
        _: info::GetPropertyListResults,
    ) -> Promise<(), ::capnp::Error> {
        Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
    fn get_reservation_list(
        &mut self,
        _: info::GetReservationListParams,
        _: info::GetReservationListResults,
    ) -> Promise<(), ::capnp::Error> {
        Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
}

impl UseServer for Machine {
    fn use_(&mut self, _: use_::UseParams, _: use_::UseResults) -> Promise<(), ::capnp::Error> {
        let resource = self.resource.clone();
        let session = self.session.clone();
        Promise::from_future(async move {
            let user = session.get_user_ref();
            resource.try_update(session, Status::InUse(user)).await;
            Ok(())
        })
    }

    fn reserve(
        &mut self,
        _: use_::ReserveParams,
        _: use_::ReserveResults,
    ) -> Promise<(), ::capnp::Error> {
        let resource = self.resource.clone();
        let session = self.session.clone();
        Promise::from_future(async move {
            let user = session.get_user_ref();
            resource
                .try_update(session, Status::Reserved(user))
                .await;
            Ok(())
        })
    }

    fn reserveto(
        &mut self,
        _: use_::ReservetoParams,
        _: use_::ReservetoResults,
    ) -> Promise<(), ::capnp::Error> {
        Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
}

impl InUseServer for Machine {
    fn give_back(
        &mut self,
        _: inuse::GiveBackParams,
        _: inuse::GiveBackResults,
    ) -> Promise<(), ::capnp::Error> {
        let resource = self.resource.clone();
        let session = self.session.clone();
        Promise::from_future(async move {
            resource.give_back(session.clone()).await;
            Ok(())
        })
    }

    fn send_raw_data(
        &mut self,
        _: inuse::SendRawDataParams,
        _: inuse::SendRawDataResults,
    ) -> Promise<(), ::capnp::Error> {
        Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
}

impl CheckServer for Machine {
    fn check(
        &mut self,
        _: check::CheckParams,
        _: check::CheckResults,
    ) -> Promise<(), ::capnp::Error> {
        Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }

    fn reject(
        &mut self,
        _: check::RejectParams,
        _: check::RejectResults,
    ) -> Promise<(), ::capnp::Error> {
        Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
}

impl ManageServer for Machine {
    fn get_machine_info_extended(
        &mut self,
        _: manage::GetMachineInfoExtendedParams,
        mut result: manage::GetMachineInfoExtendedResults,
    ) -> Promise<(), ::capnp::Error> {
        let mut builder = result.get();
        let user = User::new_self(self.session.clone());
        user.build_optional(self.resource.get_current_user(), builder.reborrow().init_current_user());
        user.build_optional(self.resource.get_previous_user(), builder.init_last_user());
        Promise::ok(())
    }
    fn set_property(
        &mut self,
        _: manage::SetPropertyParams,
        _: manage::SetPropertyResults,
    ) -> Promise<(), ::capnp::Error> {
        Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
    fn remove_property(
        &mut self,
        _: manage::RemovePropertyParams,
        _: manage::RemovePropertyResults,
    ) -> Promise<(), ::capnp::Error> {
        Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }

    fn force_use(
        &mut self,
        _: manage::ForceUseParams,
        _: manage::ForceUseResults,
    ) -> Promise<(), ::capnp::Error> {
        let resource = self.resource.clone();
        let session = self.session.clone();
        Promise::from_future(async move {
            resource
                .force_set(Status::InUse(session.get_user_ref()))
                .await;
            Ok(())
        })
    }

    fn force_free(
        &mut self,
        _: manage::ForceFreeParams,
        _: manage::ForceFreeResults,
    ) -> Promise<(), ::capnp::Error> {
        let resource = self.resource.clone();
        let _session = self.session.clone();
        Promise::from_future(async move {
            resource.force_set(Status::Free).await;
            Ok(())
        })
    }
    fn force_transfer(
        &mut self,
        _: manage::ForceTransferParams,
        _: manage::ForceTransferResults,
    ) -> Promise<(), ::capnp::Error> {
        Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }

    fn block(
        &mut self,
        _: manage::BlockParams,
        _: manage::BlockResults,
    ) -> Promise<(), ::capnp::Error> {
        let resource = self.resource.clone();
        let session = self.session.clone();
        Promise::from_future(async move {
            resource
                .force_set(Status::Blocked(session.get_user_ref()))
                .await;
            Ok(())
        })
    }
    fn disabled(
        &mut self,
        _: manage::DisabledParams,
        _: manage::DisabledResults,
    ) -> Promise<(), ::capnp::Error> {
        let resource = self.resource.clone();
        Promise::from_future(async move {
            resource.force_set(Status::Disabled).await;
            Ok(())
        })
    }
}

impl AdminServer for Machine {
    fn force_set_state(
        &mut self,
        params: admin::ForceSetStateParams,
        _: admin::ForceSetStateResults,
    ) -> Promise<(), ::capnp::Error> {
        use api::schema::machine_capnp::machine::MachineState as APIMState;
        let user = self.session.get_user_ref();
        let state = match pry!(pry!(params.get()).get_state()) {
            APIMState::Free => Status::Free,
            APIMState::Blocked => Status::Blocked(user),
            APIMState::Disabled => Status::Disabled,
            APIMState::InUse => Status::InUse(user),
            APIMState::Reserved => Status::Reserved(user),
            APIMState::ToCheck => Status::ToCheck(user),
            APIMState::Totakeover => return Promise::err(::capnp::Error::unimplemented(
                    "totakeover not implemented".to_string(),
                )),
        };
        let resource = self.resource.clone();
        Promise::from_future(async move {
            resource.force_set(state).await;
            Ok(())
        })
    }

    fn force_set_user(
        &mut self,
        _: admin::ForceSetUserParams,
        _: admin::ForceSetUserResults,
    ) -> Promise<(), ::capnp::Error> {
        Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }

    fn get_admin_property_list(
        &mut self,
        _: admin::GetAdminPropertyListParams,
        _: admin::GetAdminPropertyListResults,
    ) -> Promise<(), ::capnp::Error> {
        Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
    fn set_admin_property(
        &mut self,
        _: admin::SetAdminPropertyParams,
        _: admin::SetAdminPropertyResults,
    ) -> Promise<(), ::capnp::Error> {
        Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
    fn remove_admin_property(
        &mut self,
        _: admin::RemoveAdminPropertyParams,
        _: admin::RemoveAdminPropertyResults,
    ) -> Promise<(), ::capnp::Error> {
        Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
}
