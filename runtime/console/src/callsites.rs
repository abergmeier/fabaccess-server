use std::fmt;
use std::fmt::Formatter;
use std::ptr;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use tracing::Metadata;

pub(crate) struct Callsites<const MAX_CALLSITES: usize> {
    array: [AtomicPtr<Metadata<'static>>; MAX_CALLSITES],
    len: AtomicUsize,
}

impl<const MAX_CALLSITES: usize> Callsites<MAX_CALLSITES> {
    pub(crate) fn insert(&self, callsite: &'static Metadata<'static>) {
        if self.contains(callsite) {
            return;
        }

        let idx = self.len.fetch_add(1, Ordering::AcqRel);
        if idx <= MAX_CALLSITES {
            self.array[idx]
                .compare_exchange(
                    ptr::null_mut(),
                    callsite as *const _ as *mut _,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .expect("would have clobbered callsite array");
        } else {
            todo!("Need to spill callsite over into backup storage");
        }
    }

    pub(crate) fn contains(&self, callsite: &'static Metadata<'static>) -> bool {
        let mut idx = 0;
        let mut end = self.len.load(Ordering::Acquire);
        while {
            for cs in &self.array[idx..end] {
                let ptr = cs.load(Ordering::Acquire);
                let meta = unsafe { ptr as *const _ as &'static Metadata<'static> };
                if meta.callsite() == callsite.callsite() {
                    return true;
                }
            }

            idx = end;

            // Check if new callsites were added since we iterated
            end = self.len.load(Ordering::Acquire);
            end > idx
        } {}

        false
    }
}

impl<const MAX_CALLSITES: usize> Default for Callsites<MAX_CALLSITES> {
    fn default() -> Self {
        const NULLPTR: AtomicPtr<_> = AtomicPtr::new(ptr::null_mut());
        Self {
            array: [NULLPTR; MAX_CALLSITES],
            len: AtomicUsize::new(0),
        }
    }
}

impl<const MAX_CALLSITES: usize> fmt::Debug for Callsites<MAX_CALLSITES> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let len = self.len.load(Ordering::Acquire);
        f.debug_struct("Callsites")
            .field("MAX_CALLSITES", &MAX_CALLSITES)
            .field("array", &&self.array[..len])
            .field("len", &len)
            .finish()
    }
}
