use std::sync::Arc;

use crate::atomic::AtomicF32;

#[derive(Debug, Clone)]
pub(crate) struct Param(Arc<AtomicF32>);

impl Param {
    pub(crate) fn new(value: f32) -> Self {
        Param(Arc::new(AtomicF32::new(value)))
    }

    pub(crate) fn get(&self) -> f32 {
        self.0.get()
    }

    pub(crate) fn set(&self, value: f32) {
        self.0.set(value);
    }
}
