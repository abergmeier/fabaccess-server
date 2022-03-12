use api::machine_capnp::machine::{
    admin, admin::Server as AdminServer,
    check, check::Server as CheckServer,
    info, info::Server as InfoServer,
    in_use as inuse, in_use::Server as InUseServer,
    manage, manage::Server as ManageServer,
    use_, use_::Server as UseServer,
};

pub struct Machine;

impl InfoServer for Machine {
    fn get_property_list(
        &mut self,
        _: info::GetPropertyListParams,
        _: info::GetPropertyListResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        ::capnp::capability::Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
    fn get_reservation_list(
        &mut self,
        _: info::GetReservationListParams,
        _: info::GetReservationListResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        ::capnp::capability::Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
}

impl UseServer for Machine {
    fn use_(
        &mut self,
        _: use_::UseParams,
        _: use_::UseResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        ::capnp::capability::Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
    fn reserve(
        &mut self,
        _: use_::ReserveParams,
        _: use_::ReserveResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        ::capnp::capability::Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
    fn reserveto(
        &mut self,
        _: use_::ReservetoParams,
        _: use_::ReservetoResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        ::capnp::capability::Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
}

impl InUseServer for Machine {
    fn give_back(
        &mut self,
        _: inuse::GiveBackParams,
        _: inuse::GiveBackResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        ::capnp::capability::Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
    fn send_raw_data(
        &mut self,
        _: inuse::SendRawDataParams,
        _: inuse::SendRawDataResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        ::capnp::capability::Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
}

impl CheckServer for Machine {
    fn check(
        &mut self,
        _: check::CheckParams,
        _: check::CheckResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        ::capnp::capability::Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
    fn reject(
        &mut self,
        _: check::RejectParams,
        _: check::RejectResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        ::capnp::capability::Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
}

impl ManageServer for Machine {
    fn get_machine_info_extended(
        &mut self,
        _: manage::GetMachineInfoExtendedParams,
        _: manage::GetMachineInfoExtendedResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        ::capnp::capability::Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
    fn set_property(
        &mut self,
        _: manage::SetPropertyParams,
        _: manage::SetPropertyResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        ::capnp::capability::Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
    fn remove_property(
        &mut self,
        _: manage::RemovePropertyParams,
        _: manage::RemovePropertyResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        ::capnp::capability::Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
    fn force_use(
        &mut self,
        _: manage::ForceUseParams,
        _: manage::ForceUseResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        ::capnp::capability::Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
    fn force_free(
        &mut self,
        _: manage::ForceFreeParams,
        _: manage::ForceFreeResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        ::capnp::capability::Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
    fn force_transfer(
        &mut self,
        _: manage::ForceTransferParams,
        _: manage::ForceTransferResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        ::capnp::capability::Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
    fn block(
        &mut self,
        _: manage::BlockParams,
        _: manage::BlockResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        ::capnp::capability::Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
    fn disabled(
        &mut self,
        _: manage::DisabledParams,
        _: manage::DisabledResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        ::capnp::capability::Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
}

impl AdminServer for Machine {
    fn force_set_state(
        &mut self,
        _: admin::ForceSetStateParams,
        _: admin::ForceSetStateResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        ::capnp::capability::Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
    fn force_set_user(
        &mut self,
        _: admin::ForceSetUserParams,
        _: admin::ForceSetUserResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        ::capnp::capability::Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
    fn get_admin_property_list(
        &mut self,
        _: admin::GetAdminPropertyListParams,
        _: admin::GetAdminPropertyListResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        ::capnp::capability::Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
    fn set_admin_property(
        &mut self,
        _: admin::SetAdminPropertyParams,
        _: admin::SetAdminPropertyResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        ::capnp::capability::Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
    fn remove_admin_property(
        &mut self,
        _: admin::RemoveAdminPropertyParams,
        _: admin::RemoveAdminPropertyResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        ::capnp::capability::Promise::err(::capnp::Error::unimplemented(
            "method not implemented".to_string(),
        ))
    }
}
