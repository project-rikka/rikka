use std::cmp::Ordering;
use std::ops::{Index, IndexMut};
use std::rc::Rc;

use crate::view::observe::{log, reads};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Version {
    pub key: u64,
    pub revision: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Versions(Rc<[Version]>);

impl Versions {
    #[must_use]
    pub fn get(&self, index: usize) -> Option<Version> {
        self.0.get(index).copied()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

pub struct TrackedVec<T> {
    shape: u64,
    any: u64,
    items: Vec<T>,
    marks: Vec<Version>,
}

fn fresh() -> Version {
    Version {
        key: log::key(),
        revision: 0,
    }
}

impl<T> TrackedVec<T> {
    #[must_use]
    pub fn new() -> TrackedVec<T> {
        TrackedVec::from(Vec::new())
    }

    pub fn len(&self) -> usize {
        reads::read(self.shape);
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        reads::read(self.shape);
        let item = self.items.get(index)?;
        reads::read(self.marks[index].key);
        Some(item)
    }

    pub fn first(&self) -> Option<&T> {
        self.get(0)
    }

    pub fn last(&self) -> Option<&T> {
        reads::read(self.shape);
        self.get(self.items.len().checked_sub(1)?)
    }

    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.as_slice().iter()
    }

    pub fn as_slice(&self) -> &[T] {
        reads::read(self.shape);
        reads::read(self.any);
        &self.items
    }

    pub fn version(&self, index: usize) -> Option<Version> {
        reads::read(self.shape);
        let version = *self.marks.get(index)?;
        reads::read(version.key);
        Some(version)
    }

    pub fn versions(&self) -> Versions {
        reads::read(self.shape);
        reads::read(self.any);
        Versions(Rc::from(self.marks.as_slice()))
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        let item = self.items.get_mut(index)?;
        let mark = &mut self.marks[index];
        mark.revision += 1;
        log::changed(mark.key);
        log::changed(self.any);
        Some(item)
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, T> {
        for mark in &mut self.marks {
            mark.revision += 1;
        }
        self.reshaped();
        self.items.iter_mut()
    }

    pub fn set(&mut self, index: usize, value: T)
    where
        T: PartialEq,
    {
        if self.items.get(index).is_some_and(|item| *item != value)
            && let Some(item) = self.get_mut(index)
        {
            *item = value;
        }
    }

    pub fn push(&mut self, value: T) {
        self.items.push(value);
        self.marks.push(fresh());
        self.reshaped();
    }

    pub fn pop(&mut self) -> Option<T> {
        let value = self.items.pop()?;
        self.marks.pop();
        self.reshaped();
        Some(value)
    }

    pub fn insert(&mut self, index: usize, value: T) {
        self.items.insert(index, value);
        self.marks.insert(index, fresh());
        self.reshaped();
    }

    pub fn remove(&mut self, index: usize) -> T {
        let value = self.items.remove(index);
        self.marks.remove(index);
        self.reshaped();
        value
    }

    pub fn clear(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.items.clear();
        self.marks.clear();
        self.reshaped();
    }

    pub fn truncate(&mut self, len: usize) {
        if len >= self.items.len() {
            return;
        }
        self.items.truncate(len);
        self.marks.truncate(len);
        self.reshaped();
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&T) -> bool) {
        let retained: Vec<bool> = self.items.iter().map(&mut keep).collect();
        if retained.iter().all(|retained| *retained) {
            return;
        }
        let mut verdicts = retained.iter();
        self.items
            .retain(|_| verdicts.next().copied().unwrap_or(true));
        let mut verdicts = retained.iter();
        self.marks
            .retain(|_| verdicts.next().copied().unwrap_or(true));
        self.reshaped();
    }

    pub fn swap(&mut self, a: usize, b: usize) {
        if a == b {
            return;
        }
        self.items.swap(a, b);
        self.marks.swap(a, b);
        self.reshaped();
    }

    pub fn sort_by(&mut self, mut compare: impl FnMut(&T, &T) -> Ordering) {
        let mut paired: Vec<(T, Version)> = std::mem::take(&mut self.items)
            .into_iter()
            .zip(std::mem::take(&mut self.marks))
            .collect();
        paired.sort_by(|(a, _), (b, _)| compare(a, b));
        (self.items, self.marks) = paired.into_iter().unzip();
        self.reshaped();
    }

    pub fn sort_by_key<K: Ord>(&mut self, mut key: impl FnMut(&T) -> K) {
        self.sort_by(|a, b| key(a).cmp(&key(b)));
    }

    pub fn replace(&mut self, items: Vec<T>) {
        self.marks = items.iter().map(|_| fresh()).collect();
        self.items = items;
        self.reshaped();
    }

    fn reshaped(&self) {
        log::changed(self.shape);
        log::changed(self.any);
    }
}

impl<T> From<Vec<T>> for TrackedVec<T> {
    fn from(items: Vec<T>) -> Self {
        TrackedVec {
            shape: log::key(),
            any: log::key(),
            marks: items.iter().map(|_| fresh()).collect(),
            items,
        }
    }
}

impl<T> FromIterator<T> for TrackedVec<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        TrackedVec::from(iter.into_iter().collect::<Vec<T>>())
    }
}

impl<T> Extend<T> for TrackedVec<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        let before = self.items.len();
        self.items.extend(iter);
        let added = self.items.len() - before;
        if added == 0 {
            return;
        }
        self.marks.extend((0..added).map(|_| fresh()));
        self.reshaped();
    }
}

impl<T> Default for TrackedVec<T> {
    fn default() -> Self {
        TrackedVec::new()
    }
}

impl<T: Clone> Clone for TrackedVec<T> {
    fn clone(&self) -> Self {
        TrackedVec::from(self.as_slice().to_vec())
    }
}

impl<T> Drop for TrackedVec<T> {
    fn drop(&mut self) {
        self.reshaped();
    }
}

impl<T> Index<usize> for TrackedVec<T> {
    type Output = T;

    fn index(&self, index: usize) -> &T {
        self.get(index).expect("index out of bounds")
    }
}

impl<T> IndexMut<usize> for TrackedVec<T> {
    fn index_mut(&mut self, index: usize) -> &mut T {
        self.get_mut(index).expect("index out of bounds")
    }
}

impl<'a, T> IntoIterator for &'a TrackedVec<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T: PartialEq> PartialEq for TrackedVec<T> {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for TrackedVec<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}
