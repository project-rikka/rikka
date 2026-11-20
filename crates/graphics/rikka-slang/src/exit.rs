use std::sync::{Condvar, Mutex, OnceLock, PoisonError};

use crate::error::{Error, Result};

struct State {
    active: usize,
    closing: bool,
}

static STATE: Mutex<State> = Mutex::new(State {
    active: 0,
    closing: false,
});

static SETTLED: Condvar = Condvar::new();

pub(crate) struct Busy(());

pub(crate) fn busy() -> Result<Busy> {
    let mut state = STATE.lock().unwrap_or_else(PoisonError::into_inner);
    if state.closing {
        return Err(Error::Unavailable("the process is exiting".to_owned()));
    }
    state.active += 1;
    Ok(Busy(()))
}

impl Drop for Busy {
    fn drop(&mut self) {
        let mut state = STATE.lock().unwrap_or_else(PoisonError::into_inner);
        state.active -= 1;
        if state.active == 0 {
            SETTLED.notify_all();
        }
    }
}

pub(crate) fn hold() -> Result<(), String> {
    match unsafe { libc::atexit(drain) } {
        0 => Ok(()),
        code => Err(format!("failed to register the exit drain: {code}")),
    }
}

pub(crate) fn warmed(warm_up: impl FnOnce() -> Result<()>) -> Result<()> {
    static WARMED: OnceLock<Result<(), String>> = OnceLock::new();
    WARMED
        .get_or_init(|| {
            warm_up()
                .map_err(|error| format!("failed to warm up libslang: {error}"))
                .and_then(|()| hold())
        })
        .clone()
        .map_err(Error::Unavailable)
}

extern "C" fn drain() {
    let mut state = STATE.lock().unwrap_or_else(PoisonError::into_inner);
    state.closing = true;
    while state.active > 0 {
        state = SETTLED.wait(state).unwrap_or_else(PoisonError::into_inner);
    }
}
