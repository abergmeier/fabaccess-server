use std::pin::Pin;
use futures::task::{Context, Poll};
use futures::future::BoxFuture;
use crate::db::user::User;
use crate::db::machine::MachineState;

pub trait Sensor {
    type State: Sized;
    fn run_sensor(&mut self, state: Option<Self::State>) -> BoxFuture<'static, (Self::State, Option<User>, MachineState)>;
}
