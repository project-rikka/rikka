use std::hash::Hash;

use rustc_hash::FxHashMap;

const MAX_EMPTY: usize = 4096;
const KEEP_FRAMES: u64 = 600;
const SWEEP_FRAMES: u64 = 120;

enum Entry<P> {
    Empty {
        last_used: u64,
    },
    Placed {
        placed: P,
        last_used: u64,
        refs: u32,
    },
}

impl<P> Entry<P> {
    fn is_empty(&self) -> bool {
        matches!(self, Entry::Empty { .. })
    }
}

pub(crate) struct Residency<K, P> {
    entries: FxHashMap<K, Entry<P>>,
    empties: usize,
    swept: u64,
    looked_over: u64,
    evictable: Vec<(K, u64)>,
    victims_from: usize,
    ranked: bool,
    ranked_at: u64,
}

impl<K, P> Default for Residency<K, P> {
    fn default() -> Self {
        Self {
            entries: FxHashMap::default(),
            empties: 0,
            swept: 0,
            looked_over: 0,
            evictable: Vec::new(),
            victims_from: 0,
            ranked: false,
            ranked_at: 0,
        }
    }
}

impl<K: Copy + Eq + Hash, P> Residency<K, P> {
    pub(crate) fn contains(&self, key: &K) -> bool {
        self.entries.contains_key(key)
    }

    pub(crate) fn acquire_n(&mut self, key: K, frame: u64, count: u32) -> Option<Option<&P>> {
        let entry = self.entries.get_mut(&key)?;
        self.ranked = false;
        match entry {
            Entry::Empty { last_used } => {
                *last_used = (*last_used).max(frame);
                Some(None)
            }
            Entry::Placed {
                placed,
                last_used,
                refs,
            } => {
                *last_used = (*last_used).max(frame);
                *refs += count;
                Some(Some(placed))
            }
        }
    }

    pub(crate) fn release_n(&mut self, key: &K, count: u32) -> bool {
        if let Some(Entry::Placed { refs, .. }) = self.entries.get_mut(key) {
            *refs = refs.saturating_sub(count);
            self.ranked = false;
            return *refs == 0;
        }
        false
    }

    pub(crate) fn release(&mut self, key: &K) -> bool {
        self.release_n(key, 1)
    }

    pub(crate) fn place(&mut self, key: K, placed: P, frame: u64) {
        let entry = Entry::Placed {
            placed,
            last_used: frame,
            refs: 0,
        };
        if matches!(self.entries.insert(key, entry), Some(Entry::Empty { .. })) {
            self.empties = self.empties.saturating_sub(1);
        }
        self.ranked = false;
    }

    pub(crate) fn remember_empty(&mut self, key: K, frame: u64, completed: u64) {
        if !matches!(
            self.entries.insert(key, Entry::Empty { last_used: frame }),
            Some(Entry::Empty { .. })
        ) {
            self.empties += 1;
        }
        if self.empties <= MAX_EMPTY || self.swept == frame {
            return;
        }
        self.swept = frame;
        self.entries.retain(|_, entry| match entry {
            Entry::Empty { last_used } => *last_used > completed,
            Entry::Placed { .. } => true,
        });
        self.count_empties();
    }

    pub(crate) fn sweep(&mut self, frame: u64, completed: u64, mut retire: impl FnMut(P)) -> bool {
        if frame < self.looked_over + SWEEP_FRAMES {
            return false;
        }
        self.looked_over = frame;
        let stale = completed.saturating_sub(KEEP_FRAMES);
        let gone = self.entries.extract_if(|_, entry| match entry {
            Entry::Placed {
                last_used, refs, ..
            } => *refs == 0 && *last_used <= stale,
            Entry::Empty { last_used } => *last_used <= stale,
        });
        for (_, entry) in gone {
            if let Entry::Placed { placed, .. } = entry {
                retire(placed);
            }
        }
        self.ranked = false;
        self.count_empties();
        true
    }

    pub(crate) fn evictable(&mut self, completed: u64) -> Evictable<'_, K, P> {
        self.rank(completed);
        while let Some((key, _)) = self.evictable.get(self.victims_from) {
            if matches!(self.entries.get(key), Some(Entry::Placed { .. })) {
                break;
            }
            self.victims_from += 1;
        }
        Evictable {
            next: self.victims_from,
            residency: self,
            completed,
        }
    }

    fn rank(&mut self, completed: u64) {
        if self.ranked && self.ranked_at >= completed {
            return;
        }
        self.ranked = true;
        self.ranked_at = completed;
        self.evictable.clear();
        self.evictable
            .extend(self.entries.iter().filter_map(|(key, entry)| match entry {
                Entry::Placed {
                    last_used, refs: 0, ..
                } if *last_used <= completed => Some((*key, *last_used)),
                _ => None,
            }));
        self.evictable
            .sort_unstable_by_key(|&(_, last_used)| last_used);
        self.victims_from = 0;
    }

    fn count_empties(&mut self) {
        self.empties = self
            .entries
            .values()
            .filter(|entry| entry.is_empty())
            .count();
    }
}

pub(crate) struct Evictable<'a, K, P> {
    residency: &'a mut Residency<K, P>,
    completed: u64,
    next: usize,
}

impl<K: Copy + Eq + Hash, P> Evictable<'_, K, P> {
    pub(crate) fn next_where(&mut self, mut fits: impl FnMut(&P) -> bool) -> Option<P> {
        let residency = &mut *self.residency;
        while let Some(&(key, _)) = residency.evictable.get(self.next) {
            self.next += 1;
            let evictable = match residency.entries.get(&key) {
                Some(Entry::Placed {
                    placed,
                    last_used,
                    refs,
                }) => *refs == 0 && *last_used <= self.completed && fits(placed),
                _ => false,
            };
            if !evictable {
                continue;
            }
            if let Some(Entry::Placed { placed, .. }) = residency.entries.remove(&key) {
                return Some(placed);
            }
        }
        None
    }
}
