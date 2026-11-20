use std::fmt;
use std::marker::PhantomData;
use std::ops::{Index, IndexMut};

use crate::{Id, IdPool};

#[derive(Clone)]
pub struct Pool<I, T> {
    values: Vec<Option<T>>,
    generations: Vec<u32>,
    indices: IdPool,
    id: PhantomData<fn() -> I>,
}

impl<I, T> Default for Pool<I, T> {
    fn default() -> Self {
        Self {
            values: Vec::new(),
            generations: Vec::new(),
            indices: IdPool::new(),
            id: PhantomData,
        }
    }
}

impl<I: Id, T: fmt::Debug> fmt::Debug for Pool<I, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<I: Id, T> Pool<I, T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, value: T) -> I {
        let index = self.indices.alloc();
        if index as usize == self.values.len() {
            self.values.push(None);
            self.generations.push(0);
        }
        self.values[index as usize] = Some(value);
        I::from_parts(index, self.generations[index as usize])
    }

    pub fn remove(&mut self, id: I) -> Option<T> {
        if !self.is_current(id) {
            return None;
        }
        let index = id.index() as usize;
        let value = self.values[index].take()?;
        self.generations[index] = self.generations[index].wrapping_add(1);
        self.indices.free(id.index());
        Some(value)
    }

    #[inline]
    pub fn get(&self, id: I) -> Option<&T> {
        if self.is_current(id) {
            self.values[id.index() as usize].as_ref()
        } else {
            None
        }
    }

    #[inline]
    pub fn get_mut(&mut self, id: I) -> Option<&mut T> {
        if self.is_current(id) {
            self.values[id.index() as usize].as_mut()
        } else {
            None
        }
    }

    #[inline]
    pub fn contains(&self, id: I) -> bool {
        self.get(id).is_some()
    }

    #[inline]
    pub fn id_at(&self, index: u32) -> Option<I> {
        self.values
            .get(index as usize)?
            .as_ref()
            .map(|_| I::from_parts(index, self.generations[index as usize]))
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.indices.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    #[inline]
    pub fn slots(&self) -> &[Option<T>] {
        &self.values
    }

    #[inline]
    pub fn slots_mut(&mut self) -> &mut [Option<T>] {
        &mut self.values
    }

    pub fn iter(&self) -> impl Iterator<Item = (I, &T)> {
        self.values
            .iter()
            .zip(&self.generations)
            .zip(0u32..)
            .filter_map(|((value, &generation), index)| {
                Some((I::from_parts(index, generation), value.as_ref()?))
            })
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (I, &mut T)> {
        self.values
            .iter_mut()
            .zip(&self.generations)
            .zip(0u32..)
            .filter_map(|((value, &generation), index)| {
                Some((I::from_parts(index, generation), value.as_mut()?))
            })
    }

    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.values.iter().flatten()
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.values.iter_mut().flatten()
    }

    pub fn reset(&mut self) {
        self.values.clear();
        self.generations.clear();
        self.indices.clear();
    }

    #[inline]
    fn is_current(&self, id: I) -> bool {
        self.generations.get(id.index() as usize) == Some(&id.generation())
    }
}

impl<I: Id, T> Index<I> for Pool<I, T> {
    type Output = T;

    #[inline]
    fn index(&self, id: I) -> &T {
        match self.get(id) {
            Some(value) => value,
            None => panic!("stale or unknown id {id:?}"),
        }
    }
}

impl<I: Id, T> IndexMut<I> for Pool<I, T> {
    #[inline]
    fn index_mut(&mut self, id: I) -> &mut T {
        match self.get_mut(id) {
            Some(value) => value,
            None => panic!("stale or unknown id {id:?}"),
        }
    }
}
