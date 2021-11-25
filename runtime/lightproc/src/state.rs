use std::fmt::{Debug, Formatter};
use std::sync::atomic::{AtomicU64, Ordering};

/// Set if the proc is scheduled for running.
///
/// A proc is considered to be scheduled whenever its `LightProc` reference exists. It is in scheduled
/// state at the moment of creation and when it gets unpaused either by its `ProcHandle` or woken
/// by a `Waker`.
///
/// This flag can't be set when the proc is completed. However, it can be set while the proc is
/// running, in which case it will be rescheduled as soon as polling finishes.
pub(crate) const SCHEDULED: StateFlags = StateFlags::SCHEDULED;

/// Set if the proc is running.
///
/// A proc is running state while its future is being polled.
///
/// This flag can't be set when the proc is completed. However, it can be in scheduled state while
/// it is running, in which case it will be rescheduled when it stops being polled.
pub(crate) const RUNNING: StateFlags = StateFlags::RUNNING;

/// Set if the proc has been completed.
///
/// This flag is set when polling returns `Poll::Ready`. The output of the future is then stored
/// inside the proc until it becomes stopped. In fact, `ProcHandle` picks the output up by marking
/// the proc as stopped.
///
/// This flag can't be set when the proc is scheduled or completed.
pub(crate) const COMPLETED: StateFlags = StateFlags::COMPLETED;

/// Set if the proc is closed.
///
/// If a proc is closed, that means its either cancelled or its output has been consumed by the
/// `ProcHandle`. A proc becomes closed when:
///
/// 1. It gets cancelled by `LightProc::cancel()` or `ProcHandle::cancel()`.
/// 2. Its output is awaited by the `ProcHandle`.
/// 3. It panics while polling the future.
/// 4. It is completed and the `ProcHandle` is dropped.
pub(crate) const CLOSED: StateFlags = StateFlags::CLOSED;

/// Set if the `ProcHandle` still exists.
///
/// The `ProcHandle` is a special case in that it is only tracked by this flag, while all other
/// proc references (`LightProc` and `Waker`s) are tracked by the reference count.
pub(crate) const HANDLE: StateFlags = StateFlags::HANDLE;

/// Set if the `ProcHandle` is awaiting the output.
///
/// This flag is set while there is a registered awaiter of type `Waker` inside the proc. When the
/// proc gets closed or completed, we need to wake the awaiter. This flag can be used as a fast
/// check that tells us if we need to wake anyone without acquiring the lock inside the proc.
pub(crate) const AWAITER: StateFlags = StateFlags::AWAITER;

/// Set if the awaiter is locked.
///
/// This lock is acquired before a new awaiter is registered or the existing one is woken.
pub(crate) const LOCKED: StateFlags = StateFlags::LOCKED;

bitflags::bitflags! {
    #[derive(Default)]
    pub struct StateFlags: u32 {
        const SCHEDULED = 1 << 0;
        const RUNNING   = 1 << 1;
        const COMPLETED = 1 << 2;
        const CLOSED    = 1 << 3;
        const HANDLE    = 1 << 4;
        const AWAITER   = 1 << 5;
        const LOCKED    = 1 << 6;
    }
}

#[repr(packed)]
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct State {
    bytes: [u8; 8]
}

impl State {
    #[inline(always)]
    pub const fn new(flags: StateFlags, references: u32) -> Self {
        let [a,b,c,d] = references.to_ne_bytes();
        let [e,f,g,h] = flags.bits.to_ne_bytes();
        Self::from_bytes([a,b,c,d,e,f,g,h])
    }


    #[inline(always)]
    pub const fn parts(self: Self) -> (StateFlags, u32) {
        let [a,b,c,d,e,f,g,h] = self.bytes;
        let refcount = u32::from_ne_bytes([a,b,c,d]);
        let state = unsafe {
            StateFlags::from_bits_unchecked(u32::from_ne_bytes([e,f,g,h]))
        };
        (state, refcount)
    }

    #[inline(always)]
    /// The lower bits in the state contain various flags representing the proc state, while the upper
    /// bits contain the reference count.
    /// Note that the reference counter only tracks the `LightProc` and `Waker`s. The `ProcHandle` is
    /// tracked separately by the `HANDLE` flag.
    pub const fn get_refcount(self) -> u32 {
        let [a,b,c,d,_,_,_,_] = self.bytes;
        u32::from_ne_bytes([a,b,c,d])
    }

    #[inline(always)]
    #[must_use]
    pub const fn set_refcount(self, refcount: u32) -> Self {
        let [a, b, c, d] = refcount.to_ne_bytes();
        let [_, _, _, _, e, f, g, h] = self.bytes;
        Self::from_bytes([a, b, c, d, e, f, g, h])
    }

    #[inline(always)]
    pub const fn get_flags(self) -> StateFlags {
        let [_, _, _, _, e, f, g, h] = self.bytes;
        unsafe { StateFlags::from_bits_unchecked(u32::from_ne_bytes([e,f,g,h])) }
    }

    #[inline(always)]
    const fn from_bytes(bytes: [u8; 8]) -> Self {
        Self { bytes }
    }

    #[inline(always)]
    const fn into_u64(self) -> u64 {
        u64::from_ne_bytes(self.bytes)
    }

    #[inline(always)]
    const fn from_u64(value: u64) -> Self {
        Self::from_bytes(value.to_ne_bytes())
    }

    #[inline(always)]
    pub const fn is_awaiter(&self) -> bool {
        self.get_flags().contains(AWAITER)
    }

    #[inline(always)]
    pub const fn is_closed(&self) -> bool {
        self.get_flags().contains(CLOSED)
    }

    #[inline(always)]
    pub const fn is_locked(&self) -> bool {
        self.get_flags().contains(LOCKED)
    }

    #[inline(always)]
    pub const fn is_scheduled(&self) -> bool {
        self.get_flags().contains(SCHEDULED)
    }

    #[inline(always)]
    pub const fn is_completed(&self) -> bool {
        self.get_flags().contains(COMPLETED)
    }

    #[inline(always)]
    pub const fn is_handle(&self) -> bool {
        self.get_flags().contains(HANDLE)
    }

    #[inline(always)]
    pub const fn is_running(&self) -> bool {
        self.get_flags().contains(RUNNING)
    }
}

impl Debug for State {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State")
            .field("flags", &self.get_flags())
            .field("references", &self.get_refcount())
            .finish()
    }
}

#[repr(transparent)]
pub struct AtomicState {
    inner: AtomicU64,
}

impl AtomicState {
    #[inline(always)]
    pub const fn new(v: State) -> Self {
        let inner = AtomicU64::new(v.into_u64());
        Self { inner }
    }

    #[inline(always)]
    pub fn load(&self, order: Ordering) -> State {
        State::from_u64(self.inner.load(order))
    }

    #[inline(always)]
    #[allow(dead_code)]
    pub fn store(&self, state: State, order: Ordering) {
        self.inner.store(state.into_u64(), order)
    }

    pub fn compare_exchange(
        &self,
        current: State,
        new: State,
        success: Ordering,
        failure: Ordering
    ) -> Result<State, State>
    {
        self.inner.compare_exchange(current.into_u64(), new.into_u64(), success, failure)
            .map(|u| State::from_u64(u))
            .map_err(|u| State::from_u64(u))
    }

    pub fn compare_exchange_weak(
        &self,
        current: State,
        new: State,
        success: Ordering,
        failure: Ordering
    ) -> Result<State, State>
    {
        self.inner.compare_exchange_weak(current.into_u64(), new.into_u64(), success, failure)
            .map(|u| State::from_u64(u))
            .map_err(|u| State::from_u64(u))
    }

    pub fn fetch_or(&self, val: StateFlags, order: Ordering) -> State {
        let [a,b,c,d] = val.bits.to_ne_bytes();
        let store = u64::from_ne_bytes([0,0,0,0,a,b,c,d]);
        State::from_u64(self.inner.fetch_or(store, order))
    }

    pub fn fetch_and(&self, val: StateFlags, order: Ordering) -> State {
        let [a,b,c,d] = val.bits.to_ne_bytes();
        let store = u64::from_ne_bytes([!0,!0,!0,!0,a,b,c,d]);
        State::from_u64(self.inner.fetch_and(store, order))
    }

    // FIXME: Do this properly
    pub fn fetch_add(&self, val: u32, order: Ordering) -> State {
        let [a,b,c,d] = val.to_ne_bytes();
        let store = u64::from_ne_bytes([a,b,c,d,0,0,0,0]);
        State::from_u64(self.inner.fetch_add(store, order))
    }

    // FIXME: Do this properly
    pub fn fetch_sub(&self, val: u32, order: Ordering) -> State {
        let [a,b,c,d] = val.to_ne_bytes();
        let store = u64::from_ne_bytes([a,b,c,d,0,0,0,0]);
        State::from_u64(self.inner.fetch_sub(store, order))
    }
}

#[cfg(test)]
mod tests {
    use crate::state::*;

    #[test]
    fn test_state_has_debug() {
        let state = SCHEDULED | AWAITER;
        println!("{:?}", state);
    }

    #[test]
    fn test_is_scheduled_returns_true() {
        let state = SCHEDULED;
        assert!(state.contains(SCHEDULED));

        let mut state2 = StateFlags::default();
        state2 |= SCHEDULED;
        assert_eq!(state, state2)
    }

    #[test]
    fn flags_work() {
        let flags = SCHEDULED;
        assert_eq!(flags, SCHEDULED);

        let flags = SCHEDULED | RUNNING;
        assert_eq!(flags, SCHEDULED | RUNNING);

        let flags = RUNNING | AWAITER | COMPLETED;
        assert_eq!(flags, RUNNING | AWAITER | COMPLETED);
    }

    #[test]
    fn test_add_sub_refcount() {
        let state = State::new(StateFlags::default(), 0);
        assert_eq!(state.get_refcount(), 0);
        let state = state.set_refcount(5);
        assert_eq!(state.get_refcount(), 5);
        let state = state.set_refcount(3);
        assert_eq!(state.get_refcount(), 3);
        let state = state.set_refcount(1);
        assert_eq!(state.get_refcount(), 1);
    }

    #[test]
    fn test_mixed_refcount() {
        let flags = SCHEDULED | RUNNING | AWAITER;
        let state = State::new(flags, 0);
        println!("{:?}", state);

        assert_eq!(state.get_refcount(), 0);

        let state = state.set_refcount(5);
        println!("{:?}", state);
        assert_eq!(state.get_refcount(), 5);

        let (mut flags, references) = state.parts();
        assert_eq!(references, 5);

        flags &= !AWAITER;
        let state = State::new(flags, references);
        println!("{:?}", state);

        assert_eq!(state.get_refcount(), 5);

        let state = state.set_refcount(3);
        println!("{:?}", state);
        assert_eq!(state.get_refcount(), 3);

        let state = state.set_refcount(1);
        println!("{:?}", state);
        assert_eq!(state.get_refcount(), 1);

        assert_eq!(state.get_flags(), SCHEDULED | RUNNING);
    }
}
