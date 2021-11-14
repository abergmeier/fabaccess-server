use crossbeam::channel::{unbounded, Sender};
use futures_executor as executor;
use lazy_static::lazy_static;
use lightproc::prelude::*;

use std::future::Future;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Copy, Clone)]
pub struct GlobalState {
    pub amount: usize,
}

fn spawn_on_thread<F, R>(future: F, gs: Arc<Mutex<GlobalState>>)
    -> RecoverableHandle<Arc<Mutex<GlobalState>>, R>
where
    F: Future<Output = R> + Send + 'static,
    R: Send + 'static,
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

    let stack = ProcStack::build(Box::new(gs))
        .initialize(Callback::wrap(|s: &mut Arc<Mutex<GlobalState>>| {
            println!("initializing");
            s.clone().lock().unwrap().amount += 1;
        }))
        .completed(Callback::wrap(|s: &mut Arc<Mutex<GlobalState>>| {
            println!("completed");
            s.clone().lock().unwrap().amount += 2;
        }));

    let schedule = |t| QUEUE.send(t).unwrap();
    let (proc, handle) = LightProc::recoverable(future, schedule, stack);
    let handle = handle
        .on_panic(|s: &mut Arc<Mutex<GlobalState>>, _e| {
            println!("panicked");
            s.clone().lock().unwrap().amount += 3;
        });

    proc.schedule();

    handle
}

fn main() {
    let gs = Arc::new(Mutex::new(GlobalState { amount: 0 }));
    let handle = spawn_on_thread(
        async {
            panic!("Panic here!");
        },
        gs.clone(),
    );

    executor::block_on(handle);

    // 0 at the start
    // +1 before the start
    // +2 after panic occurs and completion triggers
    // +3 after panic triggers
    let amount = gs.lock().unwrap().amount;
    assert_eq!(amount, 6);
    println!("Amount: {}", amount);
}
