use async_trait::async_trait;

use futures_signals::signal::Mutable;
use async_oneshot::{Sender };

use smol::channel::Receiver;

use crate::state::State;
use crate::db::{
    state::StateAccessor,
    DBError,
};

/// A resource in BFFH has to contain several different parts;
/// - Currently set state
/// - Execution state of attached actors (⇒ BFFH's job)
/// - Output of interal logic of a resource
/// ⇒ Resource logic gets read access to set state and write access to output state.
/// ⇒ state `update` happens via resource logic. This logic should do access control. If the update
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
pub trait Resource {
    /// Run whatever internal logic this resource has for the given State update, and return the
    /// new output state that this update produces.
    async fn update(&mut self, input: &State /*, internal: &State*/) -> Result<State, DBError>;
}

pub struct Update {
    pub state: State,
    pub errchan: Sender<DBError>,
}

pub struct ResourceDriver {
    // putput
    res: Box<dyn Resource>,

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

            match self.res.update(&state).await {
                Ok(outstate) => {
                    // FIXME: Send any error here to some global error collector. A failed write to
                    // the DB is not necessarily fatal, but it means that BFFH is now in an
                    // inconsistent state until a future update succeeds with writing to the DB.
                    // Not applying the new state isn't correct either since we don't know what the
                    // internal logic of the resource has done to make this happen.
                    // Another half right solution is to unwrap and recreate everything.
                    // "Best" solution would be to tell the resource to rollback their interal
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
