use crossbeam::channel;
use futures_executor as executor;
use lightproc::prelude::*;
use std::future::Future;
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

fn spawn_on_thread<F, R>(fut: F) -> (JoinHandle<()>, ProcHandle<R>)
where
    F: Future<Output = R> + Send + 'static,
    R: Send + 'static,
{
    let (sender, receiver) = channel::unbounded();

    let future = async move { fut.await };

    let schedule = move |t| sender.send(t).unwrap();
    let span = tracing::trace_span!("runtime.spawn", kind = "local");
    let (proc, handle) = LightProc::build(future, schedule, span, None);

    proc.schedule();

    let join = thread::spawn(move || {
        for proc in receiver {
            println!("Got a task: {:?}", proc);
            proc.run();
        }
    });

    (join, handle)
}

fn main() {
    let (join, handle) = spawn_on_thread(async {
        println!("Sleeping!");
        async_std::task::sleep(Duration::from_millis(100)).await;
        println!("Done sleeping 1");
        async_std::task::sleep(Duration::from_millis(100)).await;
        println!("Done sleeping 2");
        async_std::task::sleep(Duration::from_millis(100)).await;
        println!("Done sleeping 3");
        async_std::task::sleep(Duration::from_millis(100)).await;
        println!("Done sleeping 4");
        return 32;
    });
    let output = executor::block_on(handle);
    assert_eq!(output, Some(32));
    assert!(join.join().is_ok());
}
