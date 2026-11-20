use std::io;
use std::thread;
use std::time::Duration;

use rikka_sync::{Condvar, Mutex};

static RUNNING: Running = Running {
    count: Mutex::new(0),
    ended: Condvar::new(),
};

struct Running {
    count: Mutex<usize>,
    ended: Condvar,
}

struct Shift;

impl Running {
    fn enter(&'static self) -> Shift {
        *self.count.lock() += 1;
        Shift
    }
}

impl Drop for Shift {
    fn drop(&mut self) {
        let mut count = RUNNING.count.lock();
        *count -= 1;
        RUNNING.ended.notify_all();
    }
}

#[derive(Debug)]
pub(crate) struct Detached(());

impl Detached {
    pub(crate) fn spawn(
        name: &'static str,
        body: impl FnOnce() + Send + 'static,
    ) -> io::Result<Detached> {
        let shift = RUNNING.enter();
        thread::Builder::new()
            .name(name.to_owned())
            .spawn(move || {
                body();
                drop(shift);
            })?;
        Ok(Detached(()))
    }
}

pub(crate) fn drain(timeout: Duration) {
    let count = RUNNING.count.lock();
    let (left, _) = RUNNING
        .ended
        .wait_timeout_while(count, timeout, |count| *count > 0);
    if *left > 0 {
        tracing::warn!(
            count = *left,
            "detached document threads still running at exit"
        );
    }
}
