use crossbeam::channel;
use futures_executor as executor;
use lightproc::prelude::*;
use std::future::Future;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn spawn_on_thread<F, R>(fut: F) -> ProcHandle<R>
where
    F: Future<Output = R> + Send + 'static,
    R: Send + 'static,
{
    let (sender, receiver) = channel::unbounded();
    let sender = Arc::new(sender);
    let s = Arc::downgrade(&sender);

    let future = async move {
        let _ = sender;
        fut.await
    };

    let schedule = move |t| s.upgrade().unwrap().send(t).unwrap();
    let (proc, handle) = LightProc::build(
        future,
        schedule,
    );

    proc.schedule();

    thread::spawn(move || {
        for proc in receiver {
            proc.run();
        }
    });

    handle
}

fn main() {
    executor::block_on(spawn_on_thread(async {
        println!("Sleeping!");
        async_std::task::sleep(Duration::from_secs(1)).await;
        println!("Done sleeping");
    }));
}
