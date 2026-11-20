const VACANT: u64 = u64::MAX;
const MIN_SLOTS: usize = 16;

#[derive(Debug, Default)]
pub(crate) struct KeySet {
    slots: Vec<u64>,
    shift: u32,
}

impl KeySet {
    pub(crate) fn reset(&mut self, capacity: usize) {
        let size = (2 * capacity).next_power_of_two().max(MIN_SLOTS);
        self.slots.clear();
        self.slots.resize(size, VACANT);
        self.shift = u64::BITS - size.trailing_zeros();
    }

    #[inline]
    fn home(&self, key: u64) -> usize {
        (key.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> self.shift) as usize
    }

    #[inline]
    pub(crate) fn insert(&mut self, key: u64) -> bool {
        let mask = self.slots.len() - 1;
        let mut slot = self.home(key);
        loop {
            let stored = self.slots[slot];
            if stored == key {
                return false;
            }
            if stored == VACANT {
                self.slots[slot] = key;
                return true;
            }
            slot = (slot + 1) & mask;
        }
    }

    #[inline]
    pub(crate) fn contains(&self, key: u64) -> bool {
        let mask = self.slots.len() - 1;
        let mut slot = self.home(key);
        loop {
            let stored = self.slots[slot];
            if stored == key {
                return true;
            }
            if stored == VACANT {
                return false;
            }
            slot = (slot + 1) & mask;
        }
    }
}
