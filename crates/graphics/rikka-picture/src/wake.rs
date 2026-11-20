use rikka_sync::{Mutex, MutexGuard};
use std::sync::{Arc, Weak};

pub type Waking = Arc<dyn Fn() + Send + Sync>;

pub struct Sleepers(Mutex<Vec<Weak<dyn Fn() + Send + Sync>>>);

impl Sleepers {
    #[must_use]
    pub const fn new() -> Sleepers {
        Sleepers(Mutex::new(Vec::new()))
    }

    pub fn register(&self, wake: &Waking) {
        let mut sleepers = self.lock();
        sleepers.retain(|sleeper| sleeper.strong_count() > 0);
        sleepers.push(Arc::downgrade(wake));
    }

    pub fn wake_all(&self) {
        let awake: Vec<Waking> = {
            let mut sleepers = self.lock();
            sleepers.retain(|sleeper| sleeper.strong_count() > 0);
            sleepers.iter().filter_map(Weak::upgrade).collect()
        };
        for wake in awake {
            wake();
        }
    }

    fn lock(&self) -> MutexGuard<'_, Vec<Weak<dyn Fn() + Send + Sync>>> {
        self.0.lock()
    }
}

impl Default for Sleepers {
    fn default() -> Sleepers {
        Sleepers::new()
    }
}

impl std::fmt::Debug for Sleepers {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Sleepers")
            .field("registered", &self.lock().len())
            .finish()
    }
}
