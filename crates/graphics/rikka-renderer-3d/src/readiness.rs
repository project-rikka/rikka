use std::sync::atomic::{AtomicU64, Ordering};

use rikka_sync::Mutex;

static GENERATION: AtomicU64 = AtomicU64::new(1);

pub(crate) fn next_generation() -> u64 {
    GENERATION.fetch_add(1, Ordering::Relaxed)
}

type Waiter = Box<dyn FnOnce() + Send>;

#[derive(Default)]
struct State {
    ready: u64,
    waiters: Vec<(u64, Waiter)>,
}

#[derive(Default)]
pub(crate) struct Readiness {
    state: Mutex<State>,
}

impl Readiness {
    pub(crate) fn is_ready(&self, generation: u64) -> bool {
        self.state.lock().ready >= generation
    }

    pub(crate) fn on_ready(&self, generation: u64, notify: impl FnOnce() + Send + 'static) {
        let mut state = self.state.lock();
        if state.ready >= generation {
            drop(state);
            notify();
            return;
        }
        state.waiters.push((generation, Box::new(notify)));
    }

    pub(crate) fn mark_ready(&self, generation: u64) {
        let due: Vec<Waiter> = {
            let mut state = self.state.lock();
            state.ready = state.ready.max(generation);
            let ready = state.ready;
            let (due, waiting) = std::mem::take(&mut state.waiters)
                .into_iter()
                .partition(|(wanted, _)| *wanted <= ready);
            state.waiters = waiting;
            due.into_iter().map(|(_, waiter)| waiter).collect()
        };
        due.into_iter().for_each(|waiter| waiter());
    }
}

impl std::fmt::Debug for Readiness {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.state.lock();
        formatter
            .debug_struct("Readiness")
            .field("ready", &state.ready)
            .field("waiters", &state.waiters.len())
            .finish()
    }
}
