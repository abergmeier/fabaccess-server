use executor::pool;
use executor::prelude::*;
use futures_util::{stream::FuturesUnordered, Stream};
use futures_util::{FutureExt, StreamExt};
use lightproc::prelude::RecoverableHandle;
use std::io::Write;
use std::panic::resume_unwind;
use std::rc::Rc;
use std::time::Duration;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let span = tracing::span!(tracing::Level::ERROR, "panic hook").entered();
        let tid = std::thread::current().id();
        tracing::error!("Panicking ThreadId: {:?}", tid);
        tracing::error!("{}", info);
        span.exit();
    }));

    let executor = Executor::new();

    let mut handles: FuturesUnordered<RecoverableHandle<usize>> = (0..2000)
        .map(|n| {
            executor.spawn(async move {
                let m: u64 = rand::random::<u64>() % 200;
                tracing::debug!("Will sleep {} * 1 ms", m);
                // simulate some really heavy load.
                for i in 0..m {
                    async_std::task::sleep(Duration::from_millis(1)).await;
                }
                return n;
            })
        })
        .collect();
    //let handle = handles.fuse().all(|opt| async move { opt.is_some() });

    /* Futures passed to `spawn` need to be `Send` so this won't work:
     * let n = 1;
     * let unsend = spawn(async move {
     *     let rc = Rc::new(n);
     *     let tid = std::thread::current().id();
     *     tracing::info!("!Send fut {} running on thread {:?}", *rc, tid);
     *     async_std::task::sleep(Duration::from_millis(20)).await;
     *     tracing::info!("!Send fut {} still running on thread {:?}!", *rc, tid);
     *     async_std::task::sleep(Duration::from_millis(20)).await;
     *     tracing::info!("!Send fut {} still running on thread {:?}!", *rc, tid);
     *     async_std::task::sleep(Duration::from_millis(20)).await;
     *     *rc
     * });
     */

    // But you can use `spawn_local` which will make sure to never Send your task to other threads.
    // However, you can't pass it a future outright but have to hand it a generator creating the
    // future on the correct thread.
    let fut = async {
        let local_futs: FuturesUnordered<_> = (0..200)
            .map(|ref n| {
                let n = *n;
                let exe = executor.clone();
                async move {
                    exe.spawn(async {
                        let tid = std::thread::current().id();
                        tracing::info!("spawn_local({}) is on thread {:?}", n, tid);
                        exe.spawn_local(async move {
                            let rc = Rc::new(n);

                            let tid = std::thread::current().id();
                            tracing::info!("!Send fut {} running on thread {:?}", *rc, tid);

                            async_std::task::sleep(Duration::from_millis(20)).await;

                            let tid2 = std::thread::current().id();
                            tracing::info!("!Send fut {} still running on thread {:?}!", *rc, tid2);
                            assert_eq!(tid, tid2);

                            async_std::task::sleep(Duration::from_millis(20)).await;

                            let tid3 = std::thread::current().id();
                            tracing::info!("!Send fut {} still running on thread {:?}!", *rc, tid3);
                            assert_eq!(tid2, tid3);

                            *rc
                        })
                    })
                    .await
                }
            })
            .collect();
        local_futs
    };

    let a = async move {
        let mut local_futs = fut.await;
        while let Some(fut) = local_futs.next().await {
            assert!(fut.is_some());
            tracing::info!("local fut returned {:?}", fut.unwrap().await)
        }
        while let Some(a) = handles.next().await {
            assert!(a.is_some());
            tracing::info!("shared fut returned {}", a.unwrap())
        }
    };
    let b = async move {
        async_std::task::sleep(Duration::from_secs(20)).await;
        tracing::info!("This is taking too long.");
    };
    executor.run(async {
        let res = futures_util::select! {
            _ = a.fuse() => {},
            _ = b.fuse() => {},
        };
    });
}
