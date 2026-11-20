#[derive(Clone, Debug)]
pub(crate) struct Arena<T> {
    pub(crate) records: Vec<T>,
    alive: Vec<u64>,
    free: Vec<u32>,
    live: usize,
}

impl<T> Default for Arena<T> {
    fn default() -> Arena<T> {
        Arena {
            records: Vec::new(),
            alive: Vec::new(),
            free: Vec::new(),
            live: 0,
        }
    }
}

pub(crate) enum Slot {
    Fresh(u32),
    Reused(u32),
}

impl Slot {
    pub(crate) fn index(&self) -> u32 {
        match *self {
            Slot::Fresh(index) | Slot::Reused(index) => index,
        }
    }
}

impl<T> Arena<T> {
    pub(crate) fn with_capacity(capacity: usize) -> Arena<T> {
        Arena {
            records: Vec::with_capacity(capacity),
            alive: Vec::with_capacity(capacity.div_ceil(64)),
            free: Vec::new(),
            live: 0,
        }
    }

    pub(crate) fn alloc(&mut self, record: T) -> Slot {
        self.live += 1;
        match self.free.pop() {
            Some(index) => {
                self.records[index as usize] = record;
                self.alive[index as usize / 64] |= 1 << (index % 64);
                Slot::Reused(index)
            }
            None => {
                let index = self.records.len() as u32;
                self.records.push(record);
                if index as usize / 64 >= self.alive.len() {
                    self.alive.push(0);
                }
                self.alive[index as usize / 64] |= 1 << (index % 64);
                Slot::Fresh(index)
            }
        }
    }

    pub(crate) fn release(&mut self, index: u32) {
        debug_assert!(self.is_alive(index), "released a dead element");
        self.alive[index as usize / 64] &= !(1 << (index % 64));
        self.free.push(index);
        self.live -= 1;
    }

    pub(crate) fn is_alive(&self, index: u32) -> bool {
        self.alive
            .get(index as usize / 64)
            .is_some_and(|word| word & (1 << (index % 64)) != 0)
    }

    pub(crate) fn live(&self) -> usize {
        self.live
    }

    pub(crate) fn capacity(&self) -> usize {
        self.records.len()
    }

    pub(crate) fn indices(&self) -> impl Iterator<Item = u32> + '_ {
        let len = self.records.len();
        self.alive
            .iter()
            .enumerate()
            .flat_map(|(word_index, &word)| {
                let mut word = word;
                std::iter::from_fn(move || {
                    if word == 0 {
                        return None;
                    }
                    let bit = word.trailing_zeros();
                    word &= word - 1;
                    Some(word_index as u32 * 64 + bit)
                })
            })
            .take_while(move |&index| (index as usize) < len)
    }

    pub(crate) fn rebuild(records: Vec<T>) -> Arena<T> {
        let len = records.len();
        let mut alive = vec![u64::MAX; len / 64];
        if !len.is_multiple_of(64) {
            alive.push((1u64 << (len % 64)) - 1);
        }
        Arena {
            records,
            alive,
            free: Vec::new(),
            live: len,
        }
    }

    pub(crate) fn is_compact(&self) -> bool {
        self.free.is_empty()
    }
}
