
// Raw typedefs and structs for kernel communication via syscalls
pub mod ctypes;
mod syscall;
pub mod io_uring;

mod sq;
mod sqe;
mod cq;
mod cqe;

mod completion;
mod cancellation;


#[macro_export]
macro_rules! ready {
    ($e:expr $(,)?) => {
        match $e {
            std::task::Poll::Ready(t) => t,
            std::task::Poll::Pending => return std::task::Poll::Pending,
        }
    };
}