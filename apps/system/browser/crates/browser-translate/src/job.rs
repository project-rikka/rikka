use rikka_languages::Language;

use crate::unit::Unit;

#[derive(Clone, Debug)]
#[must_use]
pub struct Job {
    pub(crate) to: Language,
    pub(crate) units: Vec<Unit>,
    pub(crate) protect: bool,
}

impl Job {
    pub fn new(to: Language, units: impl Into<Vec<Unit>>) -> Job {
        Job {
            to,
            units: units.into(),
            protect: true,
        }
    }

    pub fn protect(mut self, protect: bool) -> Job {
        self.protect = protect;
        self
    }
}
