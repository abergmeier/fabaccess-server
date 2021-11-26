use api::utils::l10n_string;

use std::ops::Deref;
use capnp::capability::Promise;
use capnp::Error;
use capnp_rpc::pry;

use rsasl::{gsasl_err_to_str, SaslError, Session};
use rsasl::session::Step::{Done, NeedsMore};

use api::auth::authentication::{
    Server,
    AbortParams,
    AbortResults,
    StepParams,
    StepResults,
};
use api::auth::response::{
    Reason,
    Action,
};


pub struct Authentication {
    state: State<()>,
}

enum State<D> {
    InvalidMechanism,
    Finished,
    Aborted,
    Running(Session<D>)
}

impl Server for Authentication {
    fn step(&mut self, params: StepParams, mut results: StepResults) -> Promise<(), Error> {
        use State::*;
        match self.state {
            InvalidMechanism => {
                let mut builder = results.get();
                let mut b = builder.init_error();
                b.set_reason(Reason::BadMechanism);
                b.set_action(Action::Permanent);
            },
            Finished => {
                let mut builder = results.get();
                let mut b = builder.init_error();
                b.set_reason(Reason::Finished);
                b.set_action(Action::Permanent);
            },
            Aborted => {
                let mut builder = results.get();
                let mut b = builder.init_error();
                b.set_reason(Reason::Aborted);
                b.set_action(Action::Permanent);
            },
            Running(ref mut session) => {
                // TODO: If null what happens?
                let data: &[u8] = pry!(pry!(params.get()).get_data());

                let mut builder = results.get();
                match session.step(data) {
                    Ok(Done(Data)) => {
                        let mut b = builder.init_successful();
                    },
                    Ok(NeedsMore(Data)) => {
                        builder.set_challenge(Data.deref());
                    },
                    Err(e) => {
                        let mut b = builder.init_error();
                        b.set_reason(Reason::Aborted);
                        b.set_action(Action::Permanent);
                    }
                }
            }
        }

        Promise::ok(())
    }

    fn abort(&mut self, _: AbortParams, _: AbortResults) -> Promise<(), Error> {
        Promise::ok(())
    }
}

#[repr(transparent)]
struct SaslE {
    e: SaslError,
}

impl l10n_string::Server for SaslE {
    fn get(&mut self,
           params: l10n_string::GetParams,
           mut results: l10n_string::GetResults
    ) -> Promise<(), Error>
    {
        let lang = pry!(pry!(params.get()).get_lang());
        if lang == "en" {
            let mut builder = results.get();
            builder.set_lang("en");
            builder.set_content(gsasl_err_to_str(self.e.0));
        }

        Promise::ok(())
    }

    fn available(
        &mut self,
        _: l10n_string::AvailableParams,
        mut results: l10n_string::AvailableResults
    ) -> Promise<(), Error> {
        let mut builder = results.get();
        let mut langs = builder.init_langs(1);
        langs.set(0, "en");
        Promise::ok(())
    }
}