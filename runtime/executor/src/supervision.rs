use lightproc::GroupId;
use once_cell::sync::OnceCell;
use sharded_slab::pool::Ref;
use sharded_slab::{Clear, Pool};
use std::borrow::Borrow;
use std::cell;
use std::cell::RefCell;
use std::sync::atomic::{fence, AtomicUsize, Ordering};
use thread_local::ThreadLocal;

static REGISTRY: OnceCell<SupervisionRegistry> = OnceCell::new();

fn id_to_idx(id: &GroupId) -> usize {
    (id.into_u64() as usize).wrapping_sub(1)
}

fn idx_to_id(idx: usize) -> GroupId {
    GroupId::from_u64(idx.wrapping_add(1) as u64)
}

pub struct SupervisionRegistry {
    groups: Pool<GroupInner>,
    // TODO: would this be better as the full stack?
    current: ThreadLocal<RefCell<GroupId>>,
}

impl SupervisionRegistry {
    fn new() -> Self {
        Self {
            groups: Pool::new(),
            current: ThreadLocal::new(),
        }
    }

    pub fn with<T>(f: impl FnOnce(&Self) -> T) -> T {
        let this = REGISTRY.get_or_init(SupervisionRegistry::new);
        f(&this)
    }

    pub(crate) fn get(&self, id: &GroupId) -> Option<Ref<'_, GroupInner>> {
        self.groups.get(id_to_idx(id))
    }

    #[inline]
    pub fn current_ref(&self) -> Option<cell::Ref<GroupId>> {
        self.current.get().map(|c| c.borrow())
    }

    pub fn current() -> Option<GroupId> {
        Self::with(|this| this.current_ref().map(|id| this.clone_group(&id)))
    }

    pub(crate) fn set_current(&self, id: &GroupId) {
        self.current.get_or(|| RefCell::new(id.clone()));
    }

    pub fn new_root_group(&self) -> GroupId {
        self.new_group_inner(None)
    }

    pub fn new_group(&self) -> GroupId {
        let parent = self.current_ref().map(|id| self.clone_group(&id));
        self.new_group_inner(parent)
    }

    fn new_group_inner(&self, parent: Option<GroupId>) -> GroupId {
        tracing::trace_span!(
            target: "executor::supervision",
            "new_group"
        );
        let parent_id = parent.as_ref().map(|id| id.into_non_zero_u64());
        let idx = self
            .groups
            .create_with(|group| {
                group.parent = parent;

                let ref_cnt = group.ref_count.get_mut();
                debug_assert_eq!(0, *ref_cnt);
                *ref_cnt = 1;
            })
            .expect("Failed to allocate a new group");

        let id = idx_to_id(idx);
        tracing::trace!(
            target: "executor::supervision",
            parent = parent_id,
            id = id.into_non_zero_u64(),
            "process group created"
        );

        id
    }

    fn clone_group(&self, id: &GroupId) -> GroupId {
        tracing::trace!(
            target: "executor::supervision",
            id = id.into_u64(),
            "cloning process group"
        );
        let group = self
            .get(&id)
            .unwrap_or_else(|| panic!("tried to clone group {:?}, but no such group exists!", id));

        let ref_cnt = group.ref_count.fetch_add(1, Ordering::Relaxed);
        assert_ne!(
            0, ref_cnt,
            "tried cloning group {:?} that was already closed",
            id
        );
        id.clone()
    }

    /// Try to close the group with the given ID
    ///
    /// If this method returns `true` the Group was closed. Otherwise there are still references
    /// left open.
    fn try_close(&self, id: GroupId) -> bool {
        tracing::trace!(
            target: "executor::supervision",
            id = id.into_u64(),
            "dropping process group"
        );
        let group = match self.get(&id) {
            None if std::thread::panicking() => return false,
            None => panic!("tried to drop a ref to {:?}, but no such group exists!", id),
            Some(group) => group,
        };

        // Reference count *decreases* on the other hand must observe strong ordering — when
        let remaining = group.ref_count.fetch_sub(1, Ordering::Release);
        if !std::thread::panicking() {
            assert!(remaining < usize::MAX, "group reference count overflow");
        }
        if remaining > 1 {
            return false;
        }

        // Generate a compiler fence making sure that all other calls to `try_close` are finished
        // before the one that returns `true`.
        fence(Ordering::Acquire);
        true
    }
}

#[derive(Debug)]
pub(crate) struct GroupInner {
    parent: Option<GroupId>,
    ref_count: AtomicUsize,
}

impl GroupInner {
    #[inline]
    /// Increment the reference count of this group and return the previous value
    fn increment_refcnt(&self) -> usize {
        // Reference count increases don't need strong ordering. The increments can be done in
        // any order as long as they *do* happen.
        self.ref_count.fetch_add(1, Ordering::Relaxed)
    }
}

impl Default for GroupInner {
    fn default() -> Self {
        Self {
            parent: None,
            ref_count: AtomicUsize::new(0),
        }
    }
}

impl Clear for GroupInner {
    fn clear(&mut self) {
        // A group is always alive as long as at least one of its children is alive. So each
        // Group holds a reference to its parent if it has one. If a group is being deleted this
        // reference must be closed too, i.e. the parent reference count reduced by one.
        if let Some(parent) = self.parent.take() {
            SupervisionRegistry::with(|reg| reg.try_close(parent));
        }
    }
}
