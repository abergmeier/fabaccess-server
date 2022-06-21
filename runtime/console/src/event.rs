use tracing_core::Metadata;

pub(crate) enum Event {
    Metadata(&'static Metadata<'static>),
}
