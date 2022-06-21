use crate::aggregate::Id;
use crate::stats;
use console_api::resources;
use std::sync::Arc;
use tracing_core::Metadata;

pub(crate) enum Event {
    Metadata(&'static Metadata<'static>),
    Spawn {
        id: Id,
        metadata: &'static Metadata<'static>,
        stats: Arc<stats::TaskStats>,
        fields: Vec<console_api::Field>,
        location: Option<console_api::Location>,
    },
    Resource {
        id: Id,
        parent_id: Option<Id>,
        metadata: &'static Metadata<'static>,
        concrete_type: String,
        kind: resources::resource::Kind,
        location: Option<console_api::Location>,
        is_internal: bool,
        stats: Arc<stats::ResourceStats>,
    },
    PollOp {
        metadata: &'static Metadata<'static>,
        resource_id: Id,
        op_name: String,
        async_op_id: Id,
        task_id: Id,
        is_ready: bool,
    },
    AsyncResourceOp {
        id: Id,
        parent_id: Option<Id>,
        resource_id: Id,
        metadata: &'static Metadata<'static>,
        source: String,

        stats: Arc<stats::AsyncOpStats>,
    },
}
