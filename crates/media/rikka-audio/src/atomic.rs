use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

#[derive(Debug)]
pub(crate) struct AtomicF32(AtomicU32);

impl AtomicF32 {
    pub(crate) fn new(value: f32) -> Self {
        AtomicF32(AtomicU32::new(value.to_bits()))
    }

    pub(crate) fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }

    pub(crate) fn set(&self, value: f32) {
        self.0.store(value.to_bits(), Ordering::Relaxed);
    }
}

#[derive(Debug)]
pub(crate) struct AtomicF64(AtomicU64);

impl AtomicF64 {
    pub(crate) fn new(value: f64) -> Self {
        AtomicF64(AtomicU64::new(value.to_bits()))
    }

    pub(crate) fn get(&self) -> f64 {
        f64::from_bits(self.0.load(Ordering::Relaxed))
    }

    pub(crate) fn set(&self, value: f64) {
        self.0.store(value.to_bits(), Ordering::Relaxed);
    }
}
