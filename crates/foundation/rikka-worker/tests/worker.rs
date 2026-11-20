use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use rikka_worker::{Error, Stop, Worker};

#[derive(Debug, thiserror::Error)]
#[error("the device could not be opened")]
struct OpenFailed;

struct StopSignal(Sender<()>);

impl Stop for StopSignal {
    fn stop(&self) {
        let _ = self.0.send(());
    }
}

fn stop_signal() -> (StopSignal, Receiver<()>) {
    let (stop, stopped) = mpsc::channel();
    (StopSignal(stop), stopped)
}

#[test]
fn drop_stops_and_joins_worker() {
    let (halt, stopped) = stop_signal();
    let done = Arc::new(AtomicBool::new(false));
    let theirs = Arc::clone(&done);

    let worker = Worker::spawn("rikka-worker-test-stop", halt, move || {
        Ok::<_, OpenFailed>(move || {
            let _ = stopped.recv();
            std::thread::sleep(Duration::from_millis(20));
            theirs.store(true, Ordering::SeqCst);
        })
    })
    .unwrap();

    drop(worker);
    assert!(done.load(Ordering::SeqCst));
}

#[test]
fn startup_error_differs_from_panic() {
    let (halt, _halted) = stop_signal();
    let refused = Worker::spawn("rikka-worker-test-shut", halt, || {
        Err::<fn(), OpenFailed>(OpenFailed)
    });
    assert!(matches!(refused, Err(Error::Startup(OpenFailed))));

    let (halt, _halted) = stop_signal();
    let panicked = Worker::spawn(
        "rikka-worker-test-panic",
        halt,
        || -> Result<fn(), OpenFailed> {
            panic!("the device exploded");
        },
    );
    assert!(matches!(panicked, Err(Error::Exited)));
}
