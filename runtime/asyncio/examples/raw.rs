use std::fs::File;
use std::os::unix::prelude::AsRawFd;
use asyncio::ctypes::IORING_OP;
use asyncio::io_uring::IoUring;


fn main() {
    let file = File::open("/tmp/poem").unwrap();
    let fd = file.as_raw_fd();

    let ring = IoUring::setup(4).unwrap();
    let mut cqes = ring.cqes();

    let buf = Box::new([0u8; 4096]);
    ring.try_prepare(3, |mut sqes| {
        let mut sqe = sqes.next().unwrap();
        sqe.set_opcode(IORING_OP::READ);
        sqe.set_address(buf.as_ptr() as u64);
        sqe.set_fd(fd);
        sqe.set_len(4096);

        let mut sqe = sqes.next().unwrap();
        sqe.set_opcode(IORING_OP::NOP);
        sqe.set_userdata(0xCAFEBABE);

        let mut sqe = sqes.next().unwrap();
        sqe.set_opcode(IORING_OP::NOP);
        sqe.set_userdata(0xDEADBEEF);
    }).unwrap();
    let mut amt  = 0;
    while amt == 0 {
        amt = ring.submit().unwrap();
    }
    println!("{}", amt);

    for _ in 0..3 {
        let mut cqe = None;
        while cqe.is_none() {
            cqe = cqes.next();
        }
        let cqe = cqe.unwrap();
        println!("{:?}", cqe);
        if cqe.user_data == 0xCAFEBABE {
            println!("cafebabe");
        } else if cqe.user_data == 0xDEADBEEF {
            println!("deadbeef");
        }

        if let Ok(len) = cqe.result() {
            let out = unsafe { std::str::from_utf8_unchecked(&buf[0..len as usize]) };
            println!("{}", out);
        }
    }
}