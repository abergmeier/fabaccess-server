use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug)]
/// Unique ID Allocator
///
/// Helper to allocate numerical ids in shared contexts
pub struct IdAllocator {
    next_id: AtomicU64,
}

impl IdAllocator {
    pub fn new(next_id: u64) -> Self {
        Self { next_id: AtomicU64::new(next_id) }
    }

    /// Return a new unused ID using an atomic fetch-add
    pub fn get_next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Release)
    }
}

pub struct IdSegments {
    segments: Vec<(u64, u64)>,
}

impl IdSegments {
    pub fn new(segments: Vec<(u64, u64)>) -> Self {
        Self { segments }
    }
}