use crate::resources::modules::fabaccess::{Status};
use crate::resources::Resource;
use crate::session::SessionHandle;
use api::machine_capnp::machine::{
    admin, admin::Server as AdminServer, check, check::Server as CheckServer, in_use as inuse,
    in_use::Server as InUseServer, info, info::Server as InfoServer, manage,
    manage::Server as ManageServer, use_, use_::Server as UseServer, Builder,
};
use capnp::capability::Promise;
use capnp_rpc::pry;

#[derive(Clone)]
pub struct Machine {
    session: SessionHandle,
    resource: Resource,
}

impl Machine {
    /// Builds a machine into the given builder. Re
    pub fn build(session: SessionHandle, resource: Resource, _builder: Builder) {
        if resource.visible(&session) {}
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
            let user = session.get_user();
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
            let user = session.get_user();
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
        _: manage::GetMachineInfoExtendedResults,
    ) -> Promise<(), ::capnp::Error> {
        Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
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
                .force_set(Status::InUse(session.get_user()))
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
                .force_set(Status::Blocked(session.get_user()))
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
        let user = self.session.get_user();
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
