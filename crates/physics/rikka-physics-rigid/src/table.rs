use rikka_physics_core::IdPool;

#[derive(Debug)]
pub(crate) struct Table<T> {
    pub records: Vec<T>,
    generations: Vec<u32>,
    alive: Vec<bool>,
    ids: IdPool,
}

impl<T> Default for Table<T> {
    fn default() -> Self {
        Self {
            records: Vec::new(),
            generations: Vec::new(),
            alive: Vec::new(),
            ids: IdPool::new(),
        }
    }
}

impl<T> Table<T> {
    pub(crate) fn insert(&mut self, value: T) -> (u32, u32) {
        let index = self.ids.alloc();
        if index as usize == self.records.len() {
            self.records.push(value);
            self.generations.push(0);
            self.alive.push(true);
        } else {
            self.records[index as usize] = value;
            self.alive[index as usize] = true;
        }
        (index, self.generations[index as usize])
    }

    pub(crate) fn reserve(&mut self, fill: T) -> u32 {
        let index = self.ids.alloc();
        if index as usize == self.records.len() {
            self.records.push(fill);
            self.generations.push(0);
            self.alive.push(true);
        } else {
            self.alive[index as usize] = true;
        }
        index
    }

    pub(crate) fn remove(&mut self, index: u32) {
        let slot = index as usize;
        debug_assert!(self.alive[slot]);
        self.alive[slot] = false;
        self.generations[slot] = self.generations[slot].wrapping_add(1);
        self.ids.free(index);
    }

    #[inline]
    pub(crate) fn is_alive(&self, index: u32, generation: u32) -> bool {
        let slot = index as usize;
        slot < self.alive.len() && self.alive[slot] && self.generations[slot] == generation
    }

    #[inline]
    pub(crate) fn generation(&self, index: u32) -> u32 {
        self.generations[index as usize]
    }

    pub(crate) fn generations(&self) -> &[u32] {
        &self.generations
    }

    pub(crate) fn split_mut(&mut self) -> (&mut [T], &[u32]) {
        (&mut self.records, &self.generations)
    }

    #[inline]
    pub(crate) fn get(&self, index: u32) -> &T {
        &self.records[index as usize]
    }

    #[inline]
    pub(crate) fn get_mut(&mut self, index: u32) -> &mut T {
        &mut self.records[index as usize]
    }

    pub(crate) fn len(&self) -> usize {
        self.ids.len()
    }

    pub(crate) fn capacity(&self) -> usize {
        self.records.len()
    }

    pub(crate) fn live(&self) -> impl Iterator<Item = u32> + '_ {
        (0..self.records.len() as u32).filter(|&index| self.alive[index as usize])
    }
}
