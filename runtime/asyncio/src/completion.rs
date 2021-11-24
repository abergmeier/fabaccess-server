use std::cell::Cell;
use std::io;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::task::Waker;
use crate::cancellation::Cancellation;

// TODO: Completions for linked requests? How would you handle having multiple results? In one
//       Completion struct or using multiple? If the latter, prepare needs to set user_data
//       for all intermediary SQE explicitly.
pub struct Completion {
    state: ManuallyDrop<Box<Cell<State>>>,
}

enum State {
    Submitted(Waker),
    Completed(io::Result<i32>),
    Cancelled(Cancellation),
    Empty,
}

impl Completion {
    pub fn new(waker: Waker) -> Self {
        Self {
            state: ManuallyDrop::new(Box::new(Cell::new(State::Submitted(waker)))),
        }
    }

    pub(crate) unsafe fn from_raw(ptr: u64) -> Self {
        let ptr = ptr as usize as *mut Cell<State>;
        let state = ManuallyDrop::new(Box::from_raw(ptr));
        Self {
            state,
        }
    }

    pub fn addr(&self) -> u64 {
        self.state.as_ptr() as *const _ as usize as u64
    }

    pub fn check(self, waker: &Waker) -> Result<io::Result<i32>, Self> {
        match self.state.replace(State::Empty) {
            State::Submitted(old_waker) => {
                // If the given waker wakes a different task than the one we were constructed
                // with we must replace our waker.
                if !old_waker.will_wake(waker) {
                    self.state.replace(State::Submitted(waker.clone()));
                } else {
                    self.state.replace(State::Submitted(old_waker));
                }
                Err(self)
            },
            State::Completed(result) => {
                Ok(result)
            },
            _ => unreachable!(),
        }
    }

    pub fn cancel(self, callback: Cancellation) {
        match self.state.replace(State::Cancelled(callback)) {
            State::Completed(_) => {
                drop(self.state);
            },
            State::Submitted(_) => {
            },
            _ => unreachable!(),
        }
    }

    pub fn complete(self, result: io::Result<i32>) {
        match self.state.replace(State::Completed(result)) {
            State::Submitted(waker) => {
                waker.wake();
            },
            State::Cancelled(callback) => {
                drop(callback);
            },
            _ => unreachable!(),
        }
    }
}