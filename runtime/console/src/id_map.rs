use crate::aggregate::Include;
use crate::stats::{DroppedAt, TimeAnchor, Unsent};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing_core::span::Id;

pub(crate) trait ToProto {
    type Output;
    fn to_proto(&self, base_time: &TimeAnchor) -> Self::Output;
}

#[derive(Debug)]
pub(crate) struct IdMap<T> {
    data: HashMap<Id, T>,
}

impl<T> Default for IdMap<T> {
    fn default() -> Self {
        IdMap {
            data: HashMap::<Id, T>::new(),
        }
    }
}

impl<T: Unsent> IdMap<T> {
    pub(crate) fn insert(&mut self, id: Id, data: T) {
        self.data.insert(id, data);
    }

    pub(crate) fn since_last_update(&mut self) -> impl Iterator<Item = (&Id, &mut T)> {
        self.data.iter_mut().filter_map(|(id, data)| {
            if data.take_unsent() {
                Some((id, data))
            } else {
                None
            }
        })
    }

    pub(crate) fn all(&self) -> impl Iterator<Item = (&Id, &T)> {
        self.data.iter()
    }

    pub(crate) fn get(&self, id: &Id) -> Option<&T> {
        self.data.get(id)
    }

    pub(crate) fn as_proto_list(
        &mut self,
        include: Include,
        base_time: &TimeAnchor,
    ) -> Vec<T::Output>
    where
        T: ToProto,
    {
        match include {
            Include::UpdatedOnly => self
                .since_last_update()
                .map(|(_, d)| d.to_proto(base_time))
                .collect(),
            Include::All => self.all().map(|(_, d)| d.to_proto(base_time)).collect(),
        }
    }

    pub(crate) fn as_proto(
        &mut self,
        include: Include,
        base_time: &TimeAnchor,
    ) -> HashMap<u64, T::Output>
    where
        T: ToProto,
    {
        match include {
            Include::UpdatedOnly => self
                .since_last_update()
                .map(|(id, d)| (id.into_u64(), d.to_proto(base_time)))
                .collect(),
            Include::All => self
                .all()
                .map(|(id, d)| (id.into_u64(), d.to_proto(base_time)))
                .collect(),
        }
    }

    pub(crate) fn drop_closed<R: DroppedAt + Unsent>(
        &mut self,
        stats: &mut IdMap<R>,
        now: Instant,
        retention: Duration,
        has_watchers: bool,
    ) {
        let _span = tracing::debug_span!(
            "drop_closed",
            entity = %std::any::type_name::<T>(),
            stats = %std::any::type_name::<R>(),
        )
        .entered();

        // drop closed entities
        tracing::trace!(?retention, has_watchers, "dropping closed");

        stats.data.retain(|id, stats| {
            if let Some(dropped_at) = stats.dropped_at() {
                let dropped_for = now.checked_duration_since(dropped_at).unwrap_or_default();
                let dirty = stats.is_unsent();
                let should_drop =
                    // if there are any clients watching, retain all dirty tasks regardless of age
                    (dirty && has_watchers)
                        || dropped_for > retention;
                tracing::trace!(
                    stats.id = ?id,
                    stats.dropped_at = ?dropped_at,
                    stats.dropped_for = ?dropped_for,
                    stats.dirty = dirty,
                    should_drop,
                );
                return !should_drop;
            }

            true
        });

        // drop closed entities which no longer have stats.
        self.data.retain(|id, _| stats.data.contains_key(id));
    }
}
