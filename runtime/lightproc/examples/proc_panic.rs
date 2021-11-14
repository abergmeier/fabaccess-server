use std::any::Any;
use std::fmt::Debug;
use std::ops::Deref;
use crossbeam::channel::{unbounded, Sender};
use futures_executor as executor;
use lazy_static::lazy_static;
use lightproc::prelude::*;
use std::future::Future;
use std::thread;

fn spawn_on_thread<F, R>(future: F) -> RecoverableHandle<R>
where
    F: Future<Output = R> + Send + 'static,
    R: Debug + Send + 'static,
{
    lazy_static! {
        // A channel that holds scheduled procs.
        static ref QUEUE: Sender<LightProc> = {
            let (sender, receiver) = unbounded::<LightProc>();

            // Start the executor thread.
            thread::spawn(move || {
                for proc in receiver {
                    proc.run();
                }
            });

            sender
        };
    }

    let schedule = |t| (QUEUE.deref()).send(t).unwrap();
    let (proc, handle) = LightProc::recoverable(
        future,
        schedule
    );

    let handle = handle
        .on_panic(|err: Box<dyn Any + Send>| {
            match err.downcast::<&'static str>() {
                Ok(reason) => println!("Future panicked: {}", &reason),
                Err(err) =>
                    println!("Future panicked with a non-text reason of typeid {:?}",
                             err.type_id()),
            }
        });

    proc.schedule();

    handle
}

fn main() {
    let handle = spawn_on_thread(async {
        panic!("Panic here!");
    });

    executor::block_on(handle);

    println!("But see, despite the inner future panicking we can continue executing as normal.");
}
