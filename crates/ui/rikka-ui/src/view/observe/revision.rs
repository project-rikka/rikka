use crate::view::observe::{log, reads};

pub struct Revision {
    key: u64,
}

impl Revision {
    #[must_use]
    pub fn new() -> Revision {
        Revision { key: log::key() }
    }

    pub fn read(&self) {
        reads::read(self.key);
    }

    pub fn changed(&mut self) {
        log::changed(self.key);
    }
}

impl Default for Revision {
    fn default() -> Self {
        Revision::new()
    }
}

impl Drop for Revision {
    fn drop(&mut self) {
        log::changed(self.key);
    }
}

impl std::fmt::Debug for Revision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Revision").field("key", &self.key).finish()
    }
}
