use std::fmt::Debug;
use async_trait::async_trait;

use futures_signals::signal::Mutable;
use async_oneshot::Sender;
use async_channel::Receiver;

use state::State;
use state::db::StateAccessor;

pub mod state;
pub mod claim;
pub mod db;


/// A resources in BFFH has to contain several different parts;
/// - Currently set state
/// - Execution state of attached actors (⇒ BFFH's job)
/// - Output of interal logic of a resources
/// ⇒ Resource logic gets read access to set state and write access to output state.
/// ⇒ state `update` happens via resources logic. This logic should do access control. If the update
///   succeeds then BFFH stores those input parameters ("set" state) and results / output state.
///   Storing input parameters is relevant so that BFFH can know that an "update" is a no-op
///   without having to run the module code.
/// ⇒ in fact actors only really care about the output state, and shouldn't (need to) see "set"
/// state.
/// ⇒ example reserving:
///   - Claimant sends 'update' message with a new state
///     - Doesn't set the state until `update` has returned Ok.
///   - This runs the `update` function with that new state and the claimants user context returning
///     either an Ok or an Error.
///     - Error is returned to Claimant to show user, stop.
///   - On ok:
///     - Commit new "set" state, storing it and making it visible to all other claimants
///     - Commit new output state, storing it and notifying all connected actors / Notify
/// ⇒ BFFHs job in this whole ordeal is:
///   - Message passing primitives so that update message are queued
///   - As reliable as possible storage system for input and output state
///   - Again message passing so that updates are broadcasted to all Notify and Actors.
/// ⇒ Resource module's job is:
///   - Validating updates semantically i.e. are the types correct
///   - Check authorization of updates i.e. is this user allowed to do that
#[async_trait]
pub trait ResourceModel: Debug {
    /// Run whatever internal logic this resources has for the given State update, and return the
    /// new output state that this update produces.
    async fn on_update(&mut self, input: &State) -> Result<State, Error>;
    async fn shutdown(&mut self);
}

#[derive(Debug)]
pub struct Passthrough;
#[async_trait]
impl ResourceModel for Passthrough {
    async fn on_update(&mut self, input: &State) -> Result<State, Error> {
        Ok(input.clone())
    }

    async fn shutdown(&mut self) {}
}

/// Error type a resources implementation can produce
#[derive(Debug)]
pub enum Error {
    Internal(Box<dyn std::error::Error + Send>),
    Denied,
}

// TODO: more message context
#[derive(Debug)]
pub struct Update {
    pub state: State,
    pub errchan: Sender<Error>,
}

#[derive(Debug)]
pub struct ResourceDriver {
    // putput
    res: Box<dyn ResourceModel>,

    // input
    rx: Receiver<Update>,

    // output
    db: StateAccessor,

    signal: Mutable<State>,
}

impl ResourceDriver {
    pub async fn drive_to_end(&mut self) {
        while let Ok(update) = self.rx.recv().await {
            let state = update.state;
            let mut errchan = update.errchan;

            match self.res.on_update(&state).await {
                Ok(outstate) => {
                    // FIXME: Send any error here to some global error collector. A failed write to
                    // the DB is not necessarily fatal, but it means that BFFH is now in an
                    // inconsistent state until a future update succeeds with writing to the DB.
                    // Not applying the new state isn't correct either since we don't know what the
                    // internal logic of the resources has done to make this happen.
                    // Another half right solution is to unwrap and recreate everything.
                    // "Best" solution would be to tell the resources to rollback their interal
                    // changes on a fatal failure and then notify the Claimant, while simply trying
                    // again for temporary failures.
                    let _ = self.db.set(&state, &outstate);
                    self.signal.set(outstate);
                },
                Err(e) => {
                    let _ = errchan.send(e);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::pin::Pin;
    use std::task::Poll;
    use std::future::Future;
    use super::*;

    #[futures_test::test]
    async fn test_passthrough_is_id() {
        let inp = state::tests::gen_random();

        let mut res = Passthrough;
        let out = res.on_update(&inp).await.unwrap();
        assert_eq!(inp, out);
    }

    #[test]
    fn test_passthrough_is_always_ready() {
        let inp = State::build().finish();

        let mut res = Passthrough;
        let mut cx = futures_test::task::panic_context();
        if let Poll::Ready(_) = Pin::new(&mut res.on_update(&inp)).poll(&mut cx) {
            return;
        }
        panic!("Passthrough returned Poll::Pending")
    }
}