use std::sync::Once;

use rikka_sync::{Condvar, Mutex};

struct State {
    running: usize,
    closing: bool,
}

static STATE: Mutex<State> = Mutex::new(State {
    running: 0,
    closing: false,
});

static SETTLED: Condvar = Condvar::new();

struct Running(());

impl Drop for Running {
    fn drop(&mut self) {
        let mut state = STATE.lock();
        state.running -= 1;
        if state.running == 0 {
            SETTLED.notify_all();
        }
    }
}

pub(crate) fn run(job: impl FnOnce()) {
    static FIRST_DONE: Once = Once::new();
    let running = {
        let mut state = STATE.lock();
        if state.closing {
            return;
        }
        state.running += 1;
        Running(())
    };
    job();
    drop(running);
    FIRST_DONE.call_once(hold);
}

pub(crate) fn hold() {
    if unsafe { libc::atexit(drain) } != 0 {
        tracing::warn!("failed to register the background work exit drain");
    }
}

extern "C" fn drain() {
    let mut state = STATE.lock();
    state.closing = true;
    drop(SETTLED.wait_while(state, |state| state.running > 0));
}
