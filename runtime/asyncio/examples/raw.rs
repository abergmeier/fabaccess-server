use std::fs::File;
use asyncio::ctypes::IORING_OP;
use asyncio::io_uring::IoUring;

fn main() {
    let file = File::open("")
    let ring = IoUring::setup(64).unwrap();
    let cqes = ring.cqes();
    ring.try_prepare(1, |sqes| {
        let sqe = sqes.next().unwrap();
        sqe.set_opcode(IORING_OP::READ)
    }).unwrap();
}