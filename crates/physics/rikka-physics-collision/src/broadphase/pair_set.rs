const EMPTY: u64 = u64::MAX;
const MIN_CAPACITY: usize = 64;

#[inline]
pub fn pair_key(a: u32, b: u32) -> u64 {
    let (low, high) = if a < b { (a, b) } else { (b, a) };
    (low as u64) << 32 | high as u64
}

#[inline]
pub fn pair_of(key: u64) -> (u32, u32) {
    ((key >> 32) as u32, key as u32)
}

#[derive(Debug, Default)]
pub(crate) struct PairSet {
    slots: Vec<u64>,
    len: usize,
}

impl PairSet {
    #[inline]
    fn mask(&self) -> usize {
        self.slots.len() - 1
    }

    #[inline]
    fn home(&self, key: u64) -> usize {
        (key.wrapping_mul(0x9e37_79b9_7f4a_7c15) >> 32) as usize & self.mask()
    }

    #[inline]
    pub(crate) fn prefetch(&self, key: u64) {
        if self.len != 0 {
            rikka_physics_core::prefetch(&self.slots[self.home(key)], 8);
        }
    }

    #[inline]
    pub(crate) fn contains(&self, key: u64) -> bool {
        if self.len == 0 {
            return false;
        }
        let mask = self.mask();
        let mut slot = self.home(key);
        loop {
            match self.slots[slot] {
                EMPTY => return false,
                stored if stored == key => return true,
                _ => slot = (slot + 1) & mask,
            }
        }
    }

    pub(crate) fn remove(&mut self, key: u64) {
        if self.len == 0 {
            return;
        }
        let mask = self.mask();
        let mut slot = self.home(key);
        loop {
            match self.slots[slot] {
                EMPTY => return,
                stored if stored == key => break,
                _ => slot = (slot + 1) & mask,
            }
        }
        self.len -= 1;
        let mut hole = slot;
        let mut next = (hole + 1) & mask;
        while self.slots[next] != EMPTY {
            let home = self.home(self.slots[next]);
            if (next.wrapping_sub(home) & mask) >= (next.wrapping_sub(hole) & mask) {
                self.slots[hole] = self.slots[next];
                hole = next;
            }
            next = (next + 1) & mask;
        }
        self.slots[hole] = EMPTY;
    }

    pub(crate) fn insert_all(&mut self, added: &[u64]) {
        let needed = 2 * (self.len + added.len());
        if needed > self.slots.len() {
            self.grow(needed.next_power_of_two().max(MIN_CAPACITY));
        }
        for &key in added {
            self.place(key);
        }
    }

    fn place(&mut self, key: u64) {
        let mask = self.mask();
        let mut slot = self.home(key);
        loop {
            match self.slots[slot] {
                EMPTY => {
                    self.slots[slot] = key;
                    self.len += 1;
                    return;
                }
                stored if stored == key => return,
                _ => slot = (slot + 1) & mask,
            }
        }
    }

    fn grow(&mut self, capacity: usize) {
        let old = std::mem::replace(&mut self.slots, vec![EMPTY; capacity]);
        self.len = 0;
        for key in old {
            if key != EMPTY {
                self.place(key);
            }
        }
    }
}
