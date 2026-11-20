use std::marker::PhantomData;

use smallvec::SmallVec;

use crate::ids::ElementId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdSet<I> {
    words: Vec<u64>,
    len: usize,
    marker: PhantomData<I>,
}

impl<I> Default for IdSet<I> {
    fn default() -> IdSet<I> {
        IdSet {
            words: Vec::new(),
            len: 0,
            marker: PhantomData,
        }
    }
}

impl<I: ElementId> IdSet<I> {
    #[must_use]
    pub fn new() -> IdSet<I> {
        IdSet::default()
    }

    #[must_use]
    pub fn with_capacity(capacity: usize) -> IdSet<I> {
        IdSet {
            words: vec![0; capacity.div_ceil(64)],
            len: 0,
            marker: PhantomData,
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub fn contains(&self, id: I) -> bool {
        let index = id.index();
        self.words
            .get(index / 64)
            .is_some_and(|word| word & (1 << (index % 64)) != 0)
    }

    pub fn insert(&mut self, id: I) -> bool {
        let index = id.index();
        let word = index / 64;
        if word >= self.words.len() {
            self.words.resize(word + 1, 0);
        }
        let bit = 1u64 << (index % 64);
        let fresh = self.words[word] & bit == 0;
        self.words[word] |= bit;
        self.len += usize::from(fresh);
        fresh
    }

    pub fn remove(&mut self, id: I) -> bool {
        let index = id.index();
        let Some(word) = self.words.get_mut(index / 64) else {
            return false;
        };
        let bit = 1u64 << (index % 64);
        let present = *word & bit != 0;
        *word &= !bit;
        self.len -= usize::from(present);
        present
    }

    pub fn set(&mut self, id: I, present: bool) {
        match present {
            true => self.insert(id),
            false => self.remove(id),
        };
    }

    pub fn clear(&mut self) {
        self.words.fill(0);
        self.len = 0;
    }

    pub fn iter(&self) -> impl Iterator<Item = I> + '_ {
        self.words
            .iter()
            .enumerate()
            .flat_map(|(word_index, &word)| BitIter {
                word,
                base: word_index * 64,
            })
            .map(I::from_index)
    }

    #[must_use]
    pub fn to_vec(&self) -> Vec<I> {
        self.iter().collect()
    }

    pub fn extend(&mut self, ids: impl IntoIterator<Item = I>) {
        for id in ids {
            self.insert(id);
        }
    }

    pub fn retain(&mut self, mut keep: impl FnMut(I) -> bool) {
        let removed: Vec<I> = self.iter().filter(|&id| !keep(id)).collect();
        for id in removed {
            self.remove(id);
        }
    }
}

impl<I: ElementId> FromIterator<I> for IdSet<I> {
    fn from_iter<T: IntoIterator<Item = I>>(ids: T) -> IdSet<I> {
        let mut set = IdSet::new();
        set.extend(ids);
        set
    }
}

impl<I: ElementId> Extend<I> for IdSet<I> {
    fn extend<T: IntoIterator<Item = I>>(&mut self, ids: T) {
        for id in ids {
            self.insert(id);
        }
    }
}

struct BitIter {
    word: u64,
    base: usize,
}

impl Iterator for BitIter {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        if self.word == 0 {
            return None;
        }
        let bit = self.word.trailing_zeros() as usize;
        self.word &= self.word - 1;
        Some(self.base + bit)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct IdMap<I, T> {
    values: Vec<Option<T>>,
    len: usize,
    marker: PhantomData<I>,
}

impl<I, T> Default for IdMap<I, T> {
    fn default() -> IdMap<I, T> {
        IdMap {
            values: Vec::new(),
            len: 0,
            marker: PhantomData,
        }
    }
}

impl<I: ElementId, T> IdMap<I, T> {
    #[must_use]
    pub fn new() -> IdMap<I, T> {
        IdMap::default()
    }

    #[must_use]
    pub fn with_capacity(capacity: usize) -> IdMap<I, T> {
        let mut values = Vec::with_capacity(capacity);
        values.resize_with(capacity, || None);
        IdMap {
            values,
            len: 0,
            marker: PhantomData,
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub fn get(&self, id: I) -> Option<&T> {
        self.values.get(id.index()).and_then(Option::as_ref)
    }

    pub fn get_mut(&mut self, id: I) -> Option<&mut T> {
        self.values.get_mut(id.index()).and_then(Option::as_mut)
    }

    #[must_use]
    pub fn contains(&self, id: I) -> bool {
        self.get(id).is_some()
    }

    pub fn insert(&mut self, id: I, value: T) -> Option<T> {
        let index = id.index();
        if index >= self.values.len() {
            self.values.resize_with(index + 1, || None);
        }
        let previous = self.values[index].replace(value);
        self.len += usize::from(previous.is_none());
        previous
    }

    pub fn remove(&mut self, id: I) -> Option<T> {
        let removed = self.values.get_mut(id.index()).and_then(Option::take);
        self.len -= usize::from(removed.is_some());
        removed
    }

    pub fn entry_or_insert_with(&mut self, id: I, make: impl FnOnce() -> T) -> &mut T {
        if !self.contains(id) {
            self.insert(id, make());
        }
        self.values[id.index()]
            .as_mut()
            .expect("entry was inserted above")
    }

    pub fn clear(&mut self) {
        self.values.iter_mut().for_each(|value| *value = None);
        self.len = 0;
    }

    pub fn iter(&self) -> impl Iterator<Item = (I, &T)> + '_ {
        self.values
            .iter()
            .enumerate()
            .filter_map(|(index, value)| Some((I::from_index(index), value.as_ref()?)))
    }

    pub fn keys(&self) -> impl Iterator<Item = I> + '_ {
        self.iter().map(|(id, _)| id)
    }
}

impl<I: ElementId, T: Copy> IdMap<I, T> {
    #[must_use]
    pub fn copied(&self, id: I) -> Option<T> {
        self.get(id).copied()
    }
}

impl<I: ElementId, T> FromIterator<(I, T)> for IdMap<I, T> {
    fn from_iter<S: IntoIterator<Item = (I, T)>>(pairs: S) -> IdMap<I, T> {
        let mut map = IdMap::new();
        for (id, value) in pairs {
            map.insert(id, value);
        }
        map
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Marks<I> {
    bits: Vec<u8>,
    marker: PhantomData<I>,
}

impl<I> Default for Marks<I> {
    fn default() -> Marks<I> {
        Marks {
            bits: Vec::new(),
            marker: PhantomData,
        }
    }
}

impl<I: ElementId> Marks<I> {
    #[must_use]
    pub fn new() -> Marks<I> {
        Marks::default()
    }

    #[must_use]
    pub fn with_capacity(capacity: usize) -> Marks<I> {
        Marks {
            bits: vec![0; capacity],
            marker: PhantomData,
        }
    }

    #[must_use]
    pub fn get(&self, id: I) -> u8 {
        self.bits.get(id.index()).copied().unwrap_or(0)
    }

    #[must_use]
    pub fn test(&self, id: I, mask: u8) -> bool {
        self.get(id) & mask != 0
    }

    pub fn enable(&mut self, id: I, mask: u8) {
        *self.slot(id) |= mask;
    }

    pub fn disable(&mut self, id: I, mask: u8) {
        if let Some(bits) = self.bits.get_mut(id.index()) {
            *bits &= !mask;
        }
    }

    pub fn set(&mut self, id: I, mask: u8, enabled: bool) {
        match enabled {
            true => self.enable(id, mask),
            false => self.disable(id, mask),
        }
    }

    pub fn clear_all(&mut self, mask: u8) {
        for bits in &mut self.bits {
            *bits &= !mask;
        }
    }

    pub fn iter_with(&self, mask: u8) -> impl Iterator<Item = I> + '_ {
        self.bits
            .iter()
            .enumerate()
            .filter(move |(_, bits)| **bits & mask != 0)
            .map(|(index, _)| I::from_index(index))
    }

    fn slot(&mut self, id: I) -> &mut u8 {
        let index = id.index();
        if index >= self.bits.len() {
            self.bits.resize(index + 1, 0);
        }
        &mut self.bits[index]
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LocalSet<T>(SmallVec<[T; 16]>);

impl<T: Copy + PartialEq> LocalSet<T> {
    #[must_use]
    pub fn new() -> LocalSet<T> {
        LocalSet(SmallVec::new())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn contains(&self, value: T) -> bool {
        self.0.contains(&value)
    }

    pub fn insert(&mut self, value: T) -> bool {
        if self.0.contains(&value) {
            return false;
        }
        self.0.push(value);
        true
    }

    pub fn remove(&mut self, value: T) -> bool {
        match self.0.iter().position(|&stored| stored == value) {
            Some(position) => {
                self.0.swap_remove(position);
                true
            }
            None => false,
        }
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        self.0.iter().copied()
    }
}

impl<T: Copy + PartialEq> Extend<T> for LocalSet<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, values: I) {
        for value in values {
            self.insert(value);
        }
    }
}

impl<T: Copy + PartialEq> FromIterator<T> for LocalSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(values: I) -> LocalSet<T> {
        let mut set = LocalSet::new();
        set.extend(values);
        set
    }
}
