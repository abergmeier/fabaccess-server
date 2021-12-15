use async_trait::async_trait;
use futures_util::future::BoxFuture;

pub struct State;
pub struct UserID;
pub struct ResourceID;
pub struct Error;

pub enum UpdateError {
    /// We're not connected to anything anymore. You can't do anything about this error and the
    /// only reason why you even get it is because your future was called a last time before
    /// being shelved so best way to handle this error is to just return from your loop entirely,
    /// cleaning up any state that doesn't survive a freeze.
    Closed,

    Denied,

    Other(Box<dyn std::error::Error + Send>),
}

#[async_trait]
pub trait UpdateSink: Send {
    async fn send(&mut self, userid: Option<UserID>, state: State) -> Result<(), UpdateError>;
}

pub trait InitiatorError: std::error::Error + Send {
}

pub trait Initiator {
    fn start_for(&mut self, machine: ResourceID)
        -> BoxFuture<'static, Result<(), Box<dyn InitiatorError>>>;

    fn run(&mut self, request: &mut impl UpdateSink)
        -> BoxFuture<'static, Result<(), Box<dyn InitiatorError>>>;
}
