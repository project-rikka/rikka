use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::readiness::Readiness;

#[derive(Debug, Default)]
struct Shared {
    generation: AtomicU64,
    readiness: Readiness,
}

#[derive(Clone, Debug, Default)]
pub struct EffectReadiness {
    shared: Arc<Shared>,
}

impl EffectReadiness {
    pub fn generation(&self) -> u64 {
        self.shared.generation.load(Ordering::Acquire)
    }

    pub fn is_ready(&self) -> bool {
        self.shared.readiness.is_ready(self.generation())
    }

    pub fn on_ready(&self, notify: impl FnOnce() + Send + 'static) {
        self.shared.readiness.on_ready(self.generation(), notify);
    }

    pub(crate) fn want(&self, generation: u64) {
        self.shared.generation.store(generation, Ordering::Release);
    }

    pub(crate) fn reached(&self, generation: u64) -> bool {
        self.shared.readiness.is_ready(generation)
    }

    pub(crate) fn mark_ready(&self, generation: u64) {
        self.shared.readiness.mark_ready(generation);
    }
}
