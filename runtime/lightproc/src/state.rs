use std::sync::atomic::{AtomicU64, Ordering};

/// Set if the proc is scheduled for running.
///
/// A proc is considered to be scheduled whenever its `LightProc` reference exists. It is in scheduled
/// state at the moment of creation and when it gets unpaused either by its `ProcHandle` or woken
/// by a `Waker`.
///
/// This flag can't be set when the proc is completed. However, it can be set while the proc is
/// running, in which case it will be rescheduled as soon as polling finishes.
pub(crate) const SCHEDULED: State = State::SCHEDULED;

/// Set if the proc is running.
///
/// A proc is running state while its future is being polled.
///
/// This flag can't be set when the proc is completed. However, it can be in scheduled state while
/// it is running, in which case it will be rescheduled when it stops being polled.
pub(crate) const RUNNING: State = State::RUNNING;

/// Set if the proc has been completed.
///
/// This flag is set when polling returns `Poll::Ready`. The output of the future is then stored
/// inside the proc until it becomes stopped. In fact, `ProcHandle` picks the output up by marking
/// the proc as stopped.
///
/// This flag can't be set when the proc is scheduled or completed.
pub(crate) const COMPLETED: State = State::COMPLETED;

/// Set if the proc is closed.
///
/// If a proc is closed, that means its either cancelled or its output has been consumed by the
/// `ProcHandle`. A proc becomes closed when:
///
/// 1. It gets cancelled by `LightProc::cancel()` or `ProcHandle::cancel()`.
/// 2. Its output is awaited by the `ProcHandle`.
/// 3. It panics while polling the future.
/// 4. It is completed and the `ProcHandle` is dropped.
pub(crate) const CLOSED: State = State::CLOSED;

/// Set if the `ProcHandle` still exists.
///
/// The `ProcHandle` is a special case in that it is only tracked by this flag, while all other
/// proc references (`LightProc` and `Waker`s) are tracked by the reference count.
pub(crate) const HANDLE: State = State::HANDLE;

/// Set if the `ProcHandle` is awaiting the output.
///
/// This flag is set while there is a registered awaiter of type `Waker` inside the proc. When the
/// proc gets closed or completed, we need to wake the awaiter. This flag can be used as a fast
/// check that tells us if we need to wake anyone without acquiring the lock inside the proc.
pub(crate) const AWAITER: State = State::AWAITER;

/// Set if the awaiter is locked.
///
/// This lock is acquired before a new awaiter is registered or the existing one is woken.
pub(crate) const LOCKED: State = State::LOCKED;

/// A single reference.
///
/// The lower bits in the state contain various flags representing the proc state, while the upper
/// bits contain the reference count. The value of `REFERENCE` represents a single reference in the
/// total reference count.
///
/// Note that the reference counter only tracks the `LightProc` and `Waker`s. The `ProcHandle` is
/// tracked separately by the `HANDLE` flag.
pub(crate) const REFERENCE: State = State::REFERENCE;

bitflags::bitflags! {
    #[derive(Default)]
    pub struct State: u64 {
        const SCHEDULED = 1 << 0;
        const RUNNING   = 1 << 1;
        const COMPLETED = 1 << 2;
        const CLOSED    = 1 << 3;
        const HANDLE    = 1 << 4;
        const AWAITER   = 1 << 5;
        const LOCKED    = 1 << 6;
        const REFERENCE = 1 << 7;
    }
}

impl State {
    #[inline(always)]
    const fn new(bits: u64) -> Self {
        unsafe { Self::from_bits_unchecked(bits) }
    }

    /// Returns `true` if the future is in the pending.
    #[inline(always)]
    pub fn is_pending(&self) -> bool {
        !self.is_completed()
    }

    bitfield::bitfield_fields! {
        u64;
        #[inline(always)]
        /// A proc is considered to be scheduled whenever its `LightProc` reference exists. It is in scheduled
        /// state at the moment of creation and when it gets unpaused either by its `ProcHandle` or woken
        /// by a `Waker`.
        ///
        /// This flag can't be set when the proc is completed. However, it can be set while the proc is
        /// running, in which case it will be rescheduled as soon as polling finishes.
        pub is_scheduled, set_scheduled: 0;

        #[inline(always)]
        /// A proc is running state while its future is being polled.
        ///
        /// This flag can't be set when the proc is completed. However, it can be in scheduled state while
        /// it is running, in which case it will be rescheduled when it stops being polled.
        pub is_running, set_running: 1;

        #[inline(always)]
        /// Set if the proc has been completed.
        ///
        /// This flag is set when polling returns `Poll::Ready`. The output of the future is then stored
        /// inside the proc until it becomes stopped. In fact, `ProcHandle` picks the output up by marking
        /// the proc as stopped.
        ///
        /// This flag can't be set when the proc is scheduled or completed.
        pub is_completed, set_completed: 2;

        #[inline(always)]
        /// Set if the proc is closed.
        ///
        /// If a proc is closed, that means its either cancelled or its output has been consumed by the
        /// `ProcHandle`. A proc becomes closed when:
        ///
        /// 1. It gets cancelled by `LightProc::cancel()` or `ProcHandle::cancel()`.
        /// 2. Its output is awaited by the `ProcHandle`.
        /// 3. It panics while polling the future.
        /// 4. It is completed and the `ProcHandle` is dropped.
        pub is_closed, set_closed: 3;

        #[inline(always)]
        /// Set if the `ProcHandle` still exists.
        ///
        /// The `ProcHandle` is a special case in that it is only tracked by this flag, while all other
        /// proc references (`LightProc` and `Waker`s) are tracked by the reference count.
        pub is_handle, set_handle: 4;

        #[inline(always)]
        /// Set if the `ProcHandle` is awaiting the output.
        ///
        /// This flag is set while there is a registered awaiter of type `Waker` inside the proc. When the
        /// proc gets closed or completed, we need to wake the awaiter. This flag can be used as a fast
        /// check that tells us if we need to wake anyone without acquiring the lock inside the proc.
        pub is_awaiter, set_awaiter: 5;

        #[inline(always)]
        /// Set if the awaiter is locked.
        ///
        /// This lock is acquired before a new awaiter is registered or the existing one is woken.
        pub is_locked, set_locked: 6;

        #[inline(always)]
        /// The lower bits in the state contain various flags representing the proc state, while the upper
        /// bits contain the reference count.
        /// Note that the reference counter only tracks the `LightProc` and `Waker`s. The `ProcHandle` is
        /// tracked separately by the `HANDLE` flag.
        pub get_refcount, set_refcount: 63, 7;
    }
}

impl std::ops::Add<u64> for State {
    type Output = State;

    fn add(mut self, rhs: u64) -> Self::Output {
        self.set_refcount(self.get_refcount() + rhs);
        self
    }
}

impl std::ops::Sub<u64> for State {
    type Output = State;

    fn sub(mut self, rhs: u64) -> Self::Output {
        self.set_refcount(self.get_refcount() - rhs);
        self
    }
}

impl<T> bitfield::BitRange<T> for State
    where u64: bitfield::BitRange<T>
{
    fn bit_range(&self, msb: usize, lsb: usize) -> T {
        self.bits.bit_range(msb, lsb)
    }

    fn set_bit_range(&mut self, msb: usize, lsb: usize, value: T) {
        self.bits.set_bit_range(msb, lsb, value)
    }
}

impl Into<usize> for State {
    fn into(self) -> usize {
        self.bits as usize
    }
}

#[repr(transparent)]
pub struct AtomicState {
    inner: AtomicU64,
}

impl AtomicState {
    #[inline(always)]
    pub const fn new(v: State) -> Self {
        let inner = AtomicU64::new(v.bits);
        Self { inner }
    }

    #[inline(always)]
    pub fn load(&self, order: Ordering) -> State {
        State::new(self.inner.load(order))
    }

    #[inline(always)]
    #[allow(dead_code)]
    pub fn store(&self, val: State, order: Ordering) {
        self.inner.store(val.bits, order)
    }

    pub fn compare_exchange(
        &self,
        current: State,
        new: State,
        success: Ordering,
        failure: Ordering
    ) -> Result<State, State>
    {
        self.inner.compare_exchange(current.bits, new.bits, success, failure)
            .map(|u| State::new(u))
            .map_err(|u| State::new(u))
    }

    pub fn compare_exchange_weak(
        &self,
        current: State,
        new: State,
        success: Ordering,
        failure: Ordering
    ) -> Result<State, State>
    {
        self.inner.compare_exchange_weak(current.bits, new.bits, success, failure)
            .map(|u| State::new(u))
            .map_err(|u| State::new(u))
    }

    pub fn fetch_or(&self, val: State, order: Ordering) -> State {
        State::new(self.inner.fetch_or(val.bits, order))
    }

    pub fn fetch_and(&self, val: State, order: Ordering) -> State {
        State::new(self.inner.fetch_and(val.bits, order))
    }

    // FIXME: Do this properly
    pub fn fetch_add(&self, val: u64, order: Ordering) -> State {
        State::new(self.inner.fetch_add(val << 7, order))
    }

    // FIXME: Do this properly
    pub fn fetch_sub(&self, val: u64, order: Ordering) -> State {
        State::new(self.inner.fetch_sub(val << 7, order))
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
        assert_eq!(state.is_scheduled(), true);

        let mut state2 = State::default();
        state2.set_scheduled(true);
        assert_eq!(state, state2)
    }

    #[test]
    fn test_is_scheduled_returns_false() {
        let state = State::default();
        assert_eq!(state.is_scheduled(), false);
    }

    #[test]
    fn test_is_running_returns_true() {
        let state = RUNNING;
        assert_eq!(state.is_running(), true);
    }

    #[test]
    fn test_is_running_returns_false() {
        let state = State::default();
        assert_eq!(state.is_running(), false);
    }

    #[test]
    fn test_is_completed_returns_true() {
        let state = COMPLETED;
        assert_eq!(state.is_completed(), true);
    }

    #[test]
    fn test_is_completed_returns_false() {
        let state = State::default();
        assert_eq!(state.is_completed(), false);
    }

    #[test]
    fn test_is_closed_returns_true() {
        let state = CLOSED;
        assert_eq!(state.is_closed(), true);
    }

    #[test]
    fn test_is_closed_returns_false() {
        let state = State::default();
        assert_eq!(state.is_closed(), false);
    }

    #[test]
    fn test_is_handle_returns_true() {
        let state = HANDLE;
        assert_eq!(state.is_handle(), true);
    }

    #[test]
    fn test_is_handle_returns_false() {
        let state = State::default();
        assert_eq!(state.is_handle(), false);
    }

    #[test]
    fn test_is_awaiter_returns_true() {
        let state = AWAITER;
        assert_eq!(state.is_awaiter(), true);
    }

    #[test]
    fn test_is_awaiter_returns_false() {
        let state = State::default();
        assert_eq!(state.is_awaiter(), false);
    }

    #[test]
    fn test_is_locked_returns_true() {
        let state = LOCKED;
        assert_eq!(state.is_locked(), true);
    }

    #[test]
    fn test_is_locked_returns_false() {
        let state = State::default();
        assert_eq!(state.is_locked(), false);
    }

    #[test]
    fn test_is_pending_returns_true() {
        let state = State::default();
        assert_eq!(state.is_pending(), true);
    }

    #[test]
    fn test_is_pending_returns_false() {
        let state = COMPLETED;
        assert_eq!(state.is_pending(), false);
    }

    #[test]
    fn test_add_sub_refcount() {
        let state = State::default();
        assert_eq!(state.get_refcount(), 0);
        let state = state + 5;
        assert_eq!(state.get_refcount(), 5);
        let mut state = state - 2;
        assert_eq!(state.get_refcount(), 3);
        state.set_refcount(1);
        assert_eq!(state.get_refcount(), 1);
    }
}
