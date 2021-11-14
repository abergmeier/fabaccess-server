use std::io::Write;
use std::panic::resume_unwind;
use std::time::Duration;
use executor::pool;
use executor::prelude::*;

fn main() {
    std::panic::set_hook(Box::new(|info| {
        let tid = std::thread::current().id();
        println!("Panicking ThreadId: {:?}", tid);
        std::io::stdout().flush();
        println!("panic hook: {:?}", info);
    }));
    let tid = std::thread::current().id();
    println!("Main ThreadId: {:?}", tid);

    let handle = spawn(
        async {
            panic!("test");
        },
    );

    run(
        async {
            handle.await;
        },
        ProcStack {},
    );

    let pool = pool::get();
    let manager = pool::get_manager().unwrap();
    println!("After panic: {:?}", pool);
    println!("{:#?}", manager);

    let h = std::thread::spawn(|| {
        panic!("This is a test");
    });

    std::thread::sleep(Duration::from_secs(30));

    println!("After panic");
}
