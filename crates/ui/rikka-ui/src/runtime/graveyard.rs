use std::time::Instant;

use crate::runtime::instance::Instance;

const BURIED_AT_ONCE: usize = 64;

const MOST_WAITING: usize = 1 << 13;

#[derive(Default)]
pub(crate) struct Graveyard {
    #[expect(clippy::vec_box, reason = "instances leave the table already boxed")]
    waiting: Vec<Box<Instance>>,
}

impl Graveyard {
    pub(crate) fn hold(&mut self, instance: Box<Instance>) {
        self.waiting.push(instance);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.waiting.is_empty()
    }

    pub(crate) fn bury(&mut self, until: Instant) {
        while !self.waiting.is_empty() {
            let kept = self.waiting.len().saturating_sub(BURIED_AT_ONCE);
            self.waiting.truncate(kept);
            if Instant::now() >= until {
                return;
            }
        }
    }

    pub(crate) fn bury_overflow(&mut self) {
        if self.waiting.len() > MOST_WAITING {
            self.waiting.truncate(MOST_WAITING);
        }
    }
}
