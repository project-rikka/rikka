use std::collections::hash_map::Entry;

use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::task::fetch;

const FEW: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Producer {
    Root,
    View(u64),
    Build(u64),
}

#[derive(Debug, Default)]
pub(crate) struct Readers {
    by_key: FxHashMap<u64, Producers>,
}

impl Readers {
    pub(crate) fn add(&mut self, producer: Producer, keys: &[u64]) {
        let mut first: SmallVec<[u64; FEW]> = SmallVec::new();
        for key in keys {
            let readers = match self.by_key.entry(*key) {
                Entry::Occupied(readers) => readers.into_mut(),
                Entry::Vacant(place) => {
                    first.push(*key);
                    place.insert(Producers::default())
                }
            };
            readers.insert(producer);
        }
        fetch::hold(first);
    }

    pub(crate) fn remove(&mut self, producer: Producer, keys: &[u64]) {
        let mut last: SmallVec<[u64; FEW]> = SmallVec::new();
        for key in keys {
            let Some(readers) = self.by_key.get_mut(key) else {
                continue;
            };
            readers.remove(producer);
            if readers.list.is_empty() {
                self.by_key.remove(key);
                last.push(*key);
            }
        }
        fetch::release(last);
    }

    pub(crate) fn producers(&self, key: u64) -> &[Producer] {
        self.by_key
            .get(&key)
            .map_or(&[], |readers| readers.list.as_slice())
    }

    pub(crate) fn reads(&self, key: u64) -> bool {
        self.by_key.contains_key(&key)
    }
}

impl Drop for Readers {
    fn drop(&mut self) {
        fetch::release(self.by_key.keys().copied());
    }
}

#[derive(Debug, Default)]
struct Producers {
    list: SmallVec<[Producer; 2]>,
    places: Option<FxHashMap<Producer, usize>>,
}

impl Producers {
    fn insert(&mut self, producer: Producer) {
        match &mut self.places {
            Some(places) => {
                if let Entry::Vacant(place) = places.entry(producer) {
                    place.insert(self.list.len());
                    self.list.push(producer);
                }
            }
            None => {
                if self.list.contains(&producer) {
                    return;
                }
                self.list.push(producer);
                if self.list.len() > FEW {
                    self.places = Some(
                        self.list
                            .iter()
                            .enumerate()
                            .map(|(place, producer)| (*producer, place))
                            .collect(),
                    );
                }
            }
        }
    }

    fn remove(&mut self, producer: Producer) {
        let place = match &mut self.places {
            Some(places) => places.remove(&producer),
            None => self.list.iter().position(|reader| *reader == producer),
        };
        let Some(place) = place else {
            return;
        };
        self.list.swap_remove(place);
        if let (Some(places), Some(moved)) = (&mut self.places, self.list.get(place)) {
            places.insert(*moved, place);
        }
    }
}
