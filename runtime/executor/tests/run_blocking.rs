use std::io::Write;
use executor::run::run;
use std::thread;
use std::time::Duration;
use executor::prelude::{ProcStack, spawn};

#[cfg(feature = "tokio-runtime")]
mod tokio_tests {
    #[tokio::test]
    async fn test_run_blocking() {
        super::run_test()
    }
}

#[cfg(not(feature = "tokio-runtime"))]
mod no_tokio_tests {
    #[test]
    fn test_run_blocking() {
        super::run_test()
    }
}

fn run_test() {
    let handle = spawn(
        async {
            let duration = Duration::from_millis(1);
            thread::sleep(duration);
            //42
        },
    );

    let output = run(handle, ProcStack {});

    println!("{:?}", output);
    std::io::stdout().flush();
    assert!(output.is_some());
    std::thread::sleep(Duration::from_millis(200));
}
