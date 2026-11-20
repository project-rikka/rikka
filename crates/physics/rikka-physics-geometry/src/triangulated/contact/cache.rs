use rikka_physics_math::Aabb;

use super::triangle_hull::SatCache;
use crate::PairCache;

pub const MAX_CONTACT_TRIANGLES: usize = 256;
const MAX_COLLECTED: usize = 4 * MAX_CONTACT_TRIANGLES;
pub(crate) const WHOLE: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct FeatureKey {
    pub(crate) a: u32,
    pub(crate) b: u32,
    pub(crate) triangle: u32,
}

impl FeatureKey {
    #[inline]
    pub(crate) const fn new(a: u32, b: u32, triangle: u32) -> Self {
        Self { a, b, triangle }
    }

    #[inline]
    fn packed(&self) -> u128 {
        u128::from(self.a) << 64 | u128::from(self.b) << 32 | u128::from(self.triangle)
    }

    #[inline]
    pub(crate) fn same_children(&self, other: &Self) -> bool {
        self.a == other.a && self.b == other.b
    }

    #[inline]
    pub(crate) fn salt(&self) -> u32 {
        self.a.wrapping_mul(0x85EB_CA6B) ^ self.b.wrapping_mul(0x9E37_79B9)
    }
}

impl Ord for FeatureKey {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.packed().cmp(&other.packed())
    }
}

impl PartialOrd for FeatureKey {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct CachedFeature {
    pub(crate) key: FeatureKey,
    pub(crate) sat: SatCache,
}

#[derive(Clone, Debug)]
pub struct TriangulatedContact {
    query_bounds: Aabb,
    pub(crate) entries: Vec<CachedFeature>,
    pub(crate) pairs: Vec<PairCache>,
}

impl Default for TriangulatedContact {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Default)]
pub(crate) struct KeyCollector {
    keys: Vec<FeatureKey>,
    entries: Vec<CachedFeature>,
    pairs: Vec<PairCache>,
}

impl KeyCollector {
    #[inline]
    pub(crate) fn push(&mut self, key: FeatureKey) -> bool {
        self.keys.push(key);
        self.keys.len() < MAX_COLLECTED
    }
}

impl TriangulatedContact {
    pub fn new() -> Self {
        Self {
            query_bounds: Aabb::EMPTY,
            entries: Vec::new(),
            pairs: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        self.query_bounds = Aabb::EMPTY;
        self.entries.clear();
        self.pairs.clear();
    }

    #[inline]
    pub fn cached_features(&self) -> usize {
        self.entries.len()
    }

    #[inline]
    pub(crate) fn features(&mut self) -> (&mut [CachedFeature], &mut [PairCache]) {
        (&mut self.entries, &mut self.pairs)
    }

    pub(crate) fn groups(&mut self) -> Groups<'_> {
        Groups {
            entries: &mut self.entries,
            pairs: &mut self.pairs,
        }
    }

    pub(crate) fn refresh(
        &mut self,
        bounds: Aabb,
        margin: f32,
        paired: bool,
        collector: &mut KeyCollector,
        collect: impl FnOnce(Aabb, &mut KeyCollector),
    ) {
        if self.query_bounds.contains(&bounds) {
            return;
        }
        self.query_bounds = bounds.inflated(margin);
        collector.keys.clear();
        collect(self.query_bounds, collector);
        collector.keys.sort_unstable();
        collector.keys.dedup();
        collector.keys.truncate(MAX_CONTACT_TRIANGLES);
        collector.entries.clear();
        collector.pairs.clear();
        let carried = paired && self.pairs.len() == self.entries.len();
        let mut old = 0;
        for &key in &collector.keys {
            while self.entries.get(old).is_some_and(|entry| entry.key < key) {
                old += 1;
            }
            let found = self.entries.get(old).is_some_and(|entry| entry.key == key);
            collector.entries.push(if found {
                self.entries[old]
            } else {
                CachedFeature {
                    key,
                    sat: SatCache::default(),
                }
            });
            if paired {
                collector.pairs.push(if found && carried {
                    self.pairs[old]
                } else {
                    PairCache::EMPTY
                });
            }
            if found {
                old += 1;
            }
        }
        std::mem::swap(&mut self.entries, &mut collector.entries);
        std::mem::swap(&mut self.pairs, &mut collector.pairs);
    }
}

#[derive(Debug)]
pub(crate) struct Groups<'a> {
    entries: &'a mut [CachedFeature],
    pairs: &'a mut [PairCache],
}

impl<'a> Iterator for Groups<'a> {
    type Item = (&'a mut [CachedFeature], &'a mut [PairCache]);

    fn next(&mut self) -> Option<Self::Item> {
        let first = self.entries.first()?.key;
        let count = self
            .entries
            .iter()
            .take_while(|entry| entry.key.same_children(&first))
            .count();
        let (entries, rest) = std::mem::take(&mut self.entries).split_at_mut(count);
        self.entries = rest;
        let split = count.min(self.pairs.len());
        let (pairs, rest) = std::mem::take(&mut self.pairs).split_at_mut(split);
        self.pairs = rest;
        Some((entries, pairs))
    }
}
