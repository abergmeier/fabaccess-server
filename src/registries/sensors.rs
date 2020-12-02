use std::pin::Pin;
use futures::task::{Context, Poll};
use futures::future::BoxFuture;
use crate::db::user::User;
use crate::db::machine::MachineState;

pub trait Sensor {
    fn run_sensor(&mut self) -> BoxFuture<'static, (Option<User>, MachineState)>;
}
