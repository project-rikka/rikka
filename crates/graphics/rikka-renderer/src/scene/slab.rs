pub(crate) struct Slab<T> {
    entries: Vec<Entry<T>>,
    free: Vec<u32>,
    live: usize,
}

struct Entry<T> {
    generation: u32,
    value: Option<T>,
}

impl<T> Slab<T> {
    pub(crate) fn new() -> Self {
        Self {
            entries: Vec::new(),
            free: Vec::new(),
            live: 0,
        }
    }

    pub(crate) fn insert(&mut self, value: T) -> (u32, u32) {
        self.live += 1;
        while let Some(index) = self.free.pop() {
            let entry = &mut self.entries[index as usize];
            if entry.value.is_some() {
                continue;
            }
            entry.value = Some(value);
            return (index, entry.generation);
        }
        let index = self.entries.len() as u32;
        self.entries.push(Entry {
            generation: 0,
            value: Some(value),
        });
        (index, 0)
    }

    pub(crate) fn swap(
        &mut self,
        index: u32,
        generation: u32,
        value: &mut T,
        vacant: impl FnOnce() -> T,
    ) -> Option<u32> {
        let (was, entry) = self.claim(index, generation);
        match &mut entry.value {
            Some(kept) => {
                std::mem::swap(kept, value);
                Some(was)
            }
            None => {
                entry.value = Some(std::mem::replace(value, vacant()));
                self.live += 1;
                None
            }
        }
    }

    pub(crate) fn put(&mut self, index: u32, generation: u32, value: T) -> Option<(u32, T)> {
        let (was, entry) = self.claim(index, generation);
        let before = entry.value.replace(value);
        if before.is_none() {
            self.live += 1;
        }
        before.map(|value| (was, value))
    }

    fn claim(&mut self, index: u32, generation: u32) -> (u32, &mut Entry<T>) {
        let at = index as usize;
        while self.entries.len() <= at {
            self.free.push(self.entries.len() as u32);
            self.entries.push(Entry {
                generation: 0,
                value: None,
            });
        }
        let entry = &mut self.entries[at];
        let was = std::mem::replace(&mut entry.generation, generation);
        (was, entry)
    }

    pub(crate) fn remove(&mut self, index: u32, generation: u32) -> Option<T> {
        let entry = self.entries.get_mut(index as usize)?;
        if entry.generation != generation || entry.value.is_none() {
            return None;
        }
        let value = entry.value.take();
        entry.generation = entry.generation.wrapping_add(1);
        self.free.push(index);
        self.live -= 1;
        value
    }

    pub(crate) fn get(&self, index: u32, generation: u32) -> Option<&T> {
        let entry = self.entries.get(index as usize)?;
        if entry.generation != generation {
            return None;
        }
        entry.value.as_ref()
    }

    pub(crate) fn prefetch(&self, index: u32) {
        if let Some(entry) = self.entries.get(index as usize) {
            rikka_simd::prefetch(entry, size_of::<Entry<T>>());
        }
    }

    pub(crate) fn get_mut(&mut self, index: u32, generation: u32) -> Option<&mut T> {
        let entry = self.entries.get_mut(index as usize)?;
        if entry.generation != generation {
            return None;
        }
        entry.value.as_mut()
    }

    pub(crate) fn clear(&mut self) {
        for (index, entry) in self.entries.iter_mut().enumerate() {
            if entry.value.take().is_some() {
                entry.generation = entry.generation.wrapping_add(1);
                self.free.push(index as u32);
            }
        }
        self.live = 0;
    }

    pub(crate) fn places(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn at(&self, index: usize) -> Option<&T> {
        self.entries.get(index)?.value.as_ref()
    }

    pub(crate) fn at_mut(&mut self, index: usize) -> Option<&mut T> {
        self.entries.get_mut(index)?.value.as_mut()
    }

    pub(crate) fn id_at(&self, index: usize) -> Option<(u32, u32)> {
        let entry = self.entries.get(index)?;
        entry.value.as_ref()?;
        Some((index as u32, entry.generation))
    }

    pub(crate) fn len(&self) -> usize {
        self.live
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = ((u32, u32), &T)> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                entry
                    .value
                    .as_ref()
                    .map(|value| ((index as u32, entry.generation), value))
            })
    }
}
