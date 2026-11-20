use std::borrow::Borrow;
use std::collections::BTreeMap;
use std::sync::Arc;

use rikka_keyframe::{Around, Sequence};

use crate::key::{Keyframe, Sampled};

#[derive(Clone, Debug)]
pub struct Tracks<K: Keyframe> {
    tracks: BTreeMap<K::Id, Arc<Sequence<K>>>,
}

impl<K: Keyframe> Default for Tracks<K> {
    fn default() -> Self {
        Self {
            tracks: BTreeMap::new(),
        }
    }
}

impl<K: Keyframe> PartialEq for Tracks<K> {
    fn eq(&self, other: &Self) -> bool {
        self.tracks.len() == other.tracks.len()
            && self
                .tracks
                .iter()
                .zip(&other.tracks)
                .all(|((one, mine), (two, theirs))| {
                    one == two && (Arc::ptr_eq(mine, theirs) || mine == theirs)
                })
    }
}

impl<K: Keyframe> Tracks<K> {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    #[must_use]
    pub fn keys<Q>(&self, id: &Q) -> &[K]
    where
        K::Id: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.tracks.get(id).map_or(&[], |track| track.keys())
    }

    pub fn keyed(&self) -> impl Iterator<Item = &K::Id> {
        self.tracks.keys()
    }

    pub fn tracks(&self) -> impl Iterator<Item = (&K::Id, &[K])> {
        self.tracks.iter().map(|(id, track)| (id, track.keys()))
    }

    pub fn all_keys(&self) -> impl Iterator<Item = &K> {
        self.tracks.values().flat_map(|track| track.keys())
    }

    #[must_use]
    pub fn key<Q>(&self, id: &Q, frame: u32) -> Option<&K>
    where
        K::Id: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.tracks.get(id)?.key(frame)
    }

    #[must_use]
    pub fn held_at<Q>(&self, id: &Q, frame: u32) -> Option<&K>
    where
        K::Id: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.tracks.get(id)?.held_at(frame)
    }

    pub fn insert(&mut self, key: K) -> Option<K> {
        match self.tracks.get_mut(key.id()) {
            Some(track) => Arc::make_mut(track).insert(key),
            None => {
                let id = key.id().clone();
                let mut track = Sequence::new();
                track.insert(key);
                self.tracks.insert(id, Arc::new(track));
                None
            }
        }
    }

    pub fn remove<Q>(&mut self, id: &Q, frame: u32) -> Option<K>
    where
        K::Id: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        let track = self.tracks.get_mut(id)?;
        track.key(frame)?;
        let track = Arc::make_mut(track);
        let removed = track.remove(frame);
        if track.is_empty() {
            self.tracks.remove(id);
        }
        removed
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&K::Id) -> bool) {
        self.tracks.retain(|id, _| keep(id));
    }

    pub fn merge(&mut self, other: Self) {
        for (id, laid) in other.tracks {
            match self.tracks.get_mut(&id) {
                Some(track) => {
                    let track = Arc::make_mut(track);
                    for key in laid.keys() {
                        track.insert(key.clone());
                    }
                }
                None => {
                    self.tracks.insert(id, laid);
                }
            }
        }
    }

    #[must_use]
    pub fn last_frame_of<Q>(&self, id: &Q) -> Option<u32>
    where
        K::Id: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.tracks.get(id)?.last_frame()
    }

    #[must_use]
    pub fn last_frame(&self) -> Option<u32> {
        self.tracks
            .values()
            .filter_map(|track| track.last_frame())
            .max()
    }

    #[must_use]
    pub fn changed_from(&self, before: &Self) -> Option<u32> {
        let empty = Sequence::new();
        self.tracks
            .keys()
            .chain(before.tracks.keys())
            .filter_map(|id| {
                let after = self.tracks.get(id).map_or(&empty, |track| &**track);
                let earlier = before.tracks.get(id).map_or(&empty, |track| &**track);
                after.first_change(earlier)
            })
            .min()
    }

    pub fn touched<'a>(&'a self, before: &'a Self) -> impl Iterator<Item = &'a K::Id> {
        let changed = self
            .tracks
            .iter()
            .filter_map(|(id, track)| match before.tracks.get(id) {
                Some(earlier) if Arc::ptr_eq(track, earlier) => None,
                _ => Some(id),
            });
        let gone = before
            .tracks
            .keys()
            .filter(|id| !self.tracks.contains_key(*id));
        changed.chain(gone)
    }
}

impl<K: Sampled> Tracks<K> {
    #[must_use]
    pub fn value_at<Q>(&self, id: &Q, frame: f32) -> Option<K::Value>
    where
        K::Id: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        Some(match self.tracks.get(id)?.around(f64::from(frame))? {
            Around::Held(key) => key.value(),
            Around::Between(segment) => segment.from.toward(segment.to, segment.progress as f32),
        })
    }
}

impl<K: Keyframe> FromIterator<K> for Tracks<K> {
    fn from_iter<I: IntoIterator<Item = K>>(keys: I) -> Self {
        let mut tracks = Self::default();
        for key in keys {
            tracks.insert(key);
        }
        tracks
    }
}
