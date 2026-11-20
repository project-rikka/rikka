use crate::frame::{Frame, Framed};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sequence<K> {
    keys: Vec<K>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Around<'a, K> {
    Held(&'a K),
    Between(Segment<'a, K>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment<'a, K> {
    pub index: usize,
    pub from: &'a K,
    pub to: &'a K,
    pub offset: f64,
    pub progress: f64,
}

impl<K> Default for Sequence<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K> Sequence<K> {
    #[must_use]
    pub const fn new() -> Self {
        Self { keys: Vec::new() }
    }

    #[must_use]
    pub fn keys(&self) -> &[K] {
        &self.keys
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    #[must_use]
    pub fn first(&self) -> Option<&K> {
        self.keys.first()
    }

    #[must_use]
    pub fn last(&self) -> Option<&K> {
        self.keys.last()
    }
}

impl<K: Framed> Sequence<K> {
    pub fn find(&self, frame: K::Frame) -> Result<usize, usize> {
        self.keys.binary_search_by_key(&frame, Framed::frame)
    }

    #[must_use]
    pub fn key(&self, frame: K::Frame) -> Option<&K> {
        self.find(frame).ok().map(|index| &self.keys[index])
    }

    #[must_use]
    pub fn held_at(&self, frame: K::Frame) -> Option<&K> {
        let after = self.keys.partition_point(|key| key.frame() <= frame);
        self.keys.get(after.saturating_sub(1))
    }

    #[must_use]
    pub fn previous(&self, before: K::Frame) -> Option<&K> {
        let index = self.keys.partition_point(|key| key.frame() < before);
        index.checked_sub(1).map(|index| &self.keys[index])
    }

    #[must_use]
    pub fn next(&self, after: K::Frame) -> Option<&K> {
        let index = self.keys.partition_point(|key| key.frame() <= after);
        self.keys.get(index)
    }

    #[must_use]
    pub fn last_frame(&self) -> Option<K::Frame> {
        self.keys.last().map(Framed::frame)
    }

    pub fn insert(&mut self, key: K) -> Option<K> {
        match self.find(key.frame()) {
            Ok(index) => Some(std::mem::replace(&mut self.keys[index], key)),
            Err(index) => {
                self.keys.insert(index, key);
                None
            }
        }
    }

    pub fn remove(&mut self, frame: K::Frame) -> Option<K> {
        let index = self.find(frame).ok()?;
        Some(self.keys.remove(index))
    }

    pub fn edit<R>(&mut self, change: impl FnOnce(&mut Vec<K>) -> R) -> R {
        let changed = change(&mut self.keys);
        if !self.keys.is_sorted_by(|one, two| one.frame() < two.frame()) {
            self.keys.reverse();
            self.keys.sort_by_key(Framed::frame);
            self.keys.dedup_by_key(|key| key.frame());
        }
        changed
    }

    #[must_use]
    pub fn segment_at(&self, at: f64) -> Option<usize> {
        let last = self.keys.len().checked_sub(2)?;
        let after = self.keys.partition_point(|key| key.frame().to_f64() <= at);
        Some(after.saturating_sub(1).min(last))
    }

    #[must_use]
    pub fn around(&self, at: f64) -> Option<Around<'_, K>> {
        let (first, last) = (self.keys.first()?, self.keys.last()?);
        if at <= first.frame().to_f64() {
            return Some(Around::Held(first));
        }
        if at >= last.frame().to_f64() {
            return Some(Around::Held(last));
        }
        let index = self.segment_at(at)?;
        let (from, to) = (&self.keys[index], &self.keys[index + 1]);
        let start = from.frame().to_f64();
        let offset = at - start;
        Some(Around::Between(Segment {
            index,
            from,
            to,
            offset,
            progress: offset / (to.frame().to_f64() - start),
        }))
    }
}

impl<K: Framed + PartialEq> Sequence<K> {
    #[must_use]
    pub fn first_change(&self, before: &Self) -> Option<K::Frame> {
        if std::ptr::eq(self.keys.as_ptr(), before.keys.as_ptr())
            && self.keys.len() == before.keys.len()
        {
            return None;
        }
        let same = before
            .keys
            .iter()
            .zip(&self.keys)
            .take_while(|(one, two)| one == two)
            .count();
        if same == before.keys.len() && same == self.keys.len() {
            return None;
        }
        Some(
            same.checked_sub(1)
                .map_or(K::Frame::ZERO, |index| before.keys[index].frame()),
        )
    }
}

impl<K: Framed> FromIterator<K> for Sequence<K> {
    fn from_iter<I: IntoIterator<Item = K>>(keys: I) -> Self {
        let mut sequence = Self::new();
        sequence.edit(|laid| laid.extend(keys));
        sequence
    }
}

impl<'a, K> IntoIterator for &'a Sequence<K> {
    type Item = &'a K;
    type IntoIter = std::slice::Iter<'a, K>;

    fn into_iter(self) -> Self::IntoIter {
        self.keys.iter()
    }
}
