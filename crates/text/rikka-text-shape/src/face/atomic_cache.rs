use std::sync::atomic::{AtomicU64, Ordering};

const VALID: u64 = 1 << 63;
const KEY_MASK: u32 = 0x7FFF_FFFF;

#[derive(Debug)]
pub(crate) struct AtomicCache<const SLOTS: usize> {
    slots: Box<[AtomicU64]>,
}

impl<const SLOTS: usize> Default for AtomicCache<SLOTS> {
    fn default() -> Self {
        Self {
            slots: (0..SLOTS).map(|_| AtomicU64::new(0)).collect(),
        }
    }
}

impl<const SLOTS: usize> AtomicCache<SLOTS> {
    #[inline]
    pub(crate) fn get(&self, key: u32) -> Option<u32> {
        let entry = self.slots[key as usize % SLOTS].load(Ordering::Relaxed);
        (entry & VALID != 0 && (entry >> 32) as u32 & KEY_MASK == key).then_some(entry as u32)
    }

    #[inline]
    pub(crate) fn set(&self, key: u32, value: u32) {
        if key > KEY_MASK {
            return;
        }
        let entry = VALID | u64::from(key) << 32 | u64::from(value);
        self.slots[key as usize % SLOTS].store(entry, Ordering::Relaxed);
    }
}
