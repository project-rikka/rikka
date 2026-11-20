const VACANT: u32 = u32::MAX;
const SPREAD: u32 = 0x9E37_79B1;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ItemMap {
    keys: Vec<u32>,
    values: Vec<u32>,
    shift: u32,
}

impl ItemMap {
    pub(crate) fn reset(&mut self, items: usize) {
        let size = (2 * items).next_power_of_two().max(8);
        self.keys.clear();
        self.keys.resize(size, VACANT);
        self.values.resize(size, 0);
        self.shift = 32 - size.trailing_zeros();
    }

    #[inline]
    fn home(&self, key: u32) -> usize {
        (key.wrapping_mul(SPREAD) >> self.shift) as usize
    }

    #[inline]
    pub(crate) fn insert(&mut self, key: u32, value: u32) {
        let mask = self.keys.len() - 1;
        let mut slot = self.home(key);
        while self.keys[slot] != key && self.keys[slot] != VACANT {
            slot = (slot + 1) & mask;
        }
        self.keys[slot] = key;
        self.values[slot] = value;
    }

    #[inline]
    pub(crate) fn get(&self, key: u32) -> Option<u32> {
        if self.keys.is_empty() || key == VACANT {
            return None;
        }
        let mask = self.keys.len() - 1;
        let mut slot = self.home(key);
        loop {
            match self.keys[slot] {
                found if found == key => return Some(self.values[slot]),
                VACANT => return None,
                _ => slot = (slot + 1) & mask,
            }
        }
    }
}
