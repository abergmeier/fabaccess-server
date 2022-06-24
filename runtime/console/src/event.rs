use crate::stats;
use console_api::resources;
use std::sync::Arc;
use tracing::span;
use tracing_core::Metadata;

pub(crate) enum Event {
    Metadata(&'static Metadata<'static>),
    Spawn {
        id: span::Id,
        metadata: &'static Metadata<'static>,
        stats: Arc<stats::TaskStats>,
        fields: Vec<console_api::Field>,
        location: Option<console_api::Location>,
    },
    Resource {
        id: span::Id,
        parent_id: Option<span::Id>,
        metadata: &'static Metadata<'static>,
        concrete_type: String,
        kind: resources::resource::Kind,
        location: Option<console_api::Location>,
        is_internal: bool,
        stats: Arc<stats::ResourceStats>,
    },
    PollOp {
        metadata: &'static Metadata<'static>,
        resource_id: span::Id,
        op_name: String,
        async_op_id: span::Id,
        task_id: span::Id,
        is_ready: bool,
    },
    AsyncResourceOp {
        id: span::Id,
        parent_id: Option<span::Id>,
        resource_id: span::Id,
        metadata: &'static Metadata<'static>,
        source: String,

        stats: Arc<stats::AsyncOpStats>,
    },
}

#[derive(Clone, Debug, Copy)]
pub(crate) enum WakeOp {
    Wake { self_wake: bool },
    WakeByRef { self_wake: bool },
    Clone,
    Drop,
}

impl WakeOp {
    /// Returns `true` if `self` is a `Wake` or `WakeByRef` event.
    pub(crate) fn is_wake(self) -> bool {
        matches!(self, Self::Wake { .. } | Self::WakeByRef { .. })
    }

    pub(crate) fn self_wake(self, self_wake: bool) -> Self {
        match self {
            Self::Wake { .. } => Self::Wake { self_wake },
            Self::WakeByRef { .. } => Self::WakeByRef { self_wake },
            x => x,
        }
    }
}
