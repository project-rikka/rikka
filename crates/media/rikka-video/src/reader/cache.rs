use std::collections::BTreeMap;
use std::time::Duration;

pub(super) const MAX_AHEAD: usize = 64;
const KEPT_BEHIND: usize = 8;
const KEPT_BEYOND: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Window {
    pub(super) start: Duration,
    pub(super) end: Duration,
}

impl Window {
    pub(super) fn overlaps(&self, from: Duration, end: Duration) -> bool {
        from <= self.end && end > self.start
    }
}

pub(super) struct Entry<T> {
    pub(super) pts: Duration,
    pub(super) end: Duration,
    pub(super) value: Option<T>,
    pub(super) fit: u64,
}

pub(super) struct Cache<T> {
    entries: BTreeMap<Duration, Entry<T>>,
}

impl<T> Cache<T> {
    pub(super) fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    pub(super) fn covering(&self, at: Duration) -> Option<&Entry<T>> {
        self.entries
            .range(..=at)
            .next_back()
            .map(|(_, entry)| entry)
            .filter(|entry| at < entry.end)
    }

    pub(super) fn holds(&self, from: Duration, fit: u64) -> bool {
        self.entries
            .get(&from)
            .is_some_and(|entry| entry.fit == fit)
    }

    pub(super) fn insert(&mut self, from: Duration, entry: Entry<T>) -> Option<Entry<T>> {
        self.entries.insert(from, entry)
    }

    pub(super) fn missing(&self, window: Window, fit: u64) -> Option<Duration> {
        let mut at = window.start;
        for _ in 0..MAX_AHEAD {
            match self.covering(at) {
                Some(entry) if entry.fit == fit => {
                    if entry.end > window.end {
                        return None;
                    }
                    at = entry.end;
                }
                _ => return Some(at),
            }
        }
        None
    }

    pub(super) fn evict(&mut self, window: Window) -> Vec<Entry<T>> {
        let behind = self
            .entries
            .values()
            .filter(|entry| entry.end <= window.start)
            .count();
        let mut within = 0;
        let mut beyond = 0;
        let mut seen_behind = 0;
        let doomed: Vec<Duration> = self
            .entries
            .iter()
            .filter(|&(&from, entry)| {
                if entry.end <= window.start {
                    seen_behind += 1;
                    behind - seen_behind >= KEPT_BEHIND
                } else if window.overlaps(from, entry.end) {
                    within += 1;
                    within > MAX_AHEAD
                } else {
                    beyond += 1;
                    beyond > KEPT_BEYOND
                }
            })
            .map(|(&from, _)| from)
            .collect();
        doomed
            .into_iter()
            .filter_map(|from| self.entries.remove(&from))
            .collect()
    }

    pub(super) fn take(&mut self) -> BTreeMap<Duration, Entry<T>> {
        std::mem::take(&mut self.entries)
    }
}
