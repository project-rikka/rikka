use crate::runtime::Watchers;
use crate::view::observe::{log, reads};

pub trait Observed {
    fn observers(&self) -> &Observers;
}

pub struct Observers {
    key: u64,
    painters: Watchers,
}

impl Observers {
    #[must_use]
    pub fn new() -> Observers {
        Observers {
            key: log::key(),
            painters: Watchers::new(),
        }
    }

    pub fn read(&self) {
        match reads::recording() {
            true => reads::read(self.key),
            false => self.painters.watch_current(),
        }
    }

    pub fn painting(&self) {
        self.painters.watch_current();
    }

    pub fn changed(&self) {
        log::changed(self.key);
        self.painters.repaint_others();
    }

    pub fn reshaped(&self) {
        log::changed(self.key);
        self.painters.relayout_others();
    }
}

impl Default for Observers {
    fn default() -> Self {
        Observers::new()
    }
}

impl std::fmt::Debug for Observers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Observers")
            .field("key", &self.key)
            .field("painters", &self.painters)
            .finish()
    }
}
