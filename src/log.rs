use slog::{Drain, Logger};
use slog_async;
use slog_async::AsyncGuard;
use slog_term::{TermDecorator, FullFormat};

pub fn init() -> (Logger, AsyncGuard) {
    let decorator = TermDecorator::new().build();
    let drain = FullFormat::new(decorator).build().fuse();
    let (drain, guard) = slog_async::Async::new(drain).build_with_guard();
    let drain = drain.fuse();

    return (slog::Logger::root(drain, o!()), guard);
}
