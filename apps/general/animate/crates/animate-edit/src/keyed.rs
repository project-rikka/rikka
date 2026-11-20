use std::sync::Arc;

use animate_fluid::Emitter;
use animate_look::Look;
use animate_motion::{CameraMotion, PartKey, Take};
use animate_track::Tracks;
use animate_wind::WindMotion;
use rustc_hash::FxHashSet;

use crate::keys::{Key, KeyId, Laned};

pub trait Keyed {
    fn gather_keys(&self, ids: &FxHashSet<KeyId>) -> Vec<Key>;

    fn remove_keys(&mut self, ids: &FxHashSet<KeyId>);

    fn insert_keys(&mut self, keys: &[Key]);
}

fn gather<K: Laned>(tracks: &Tracks<K>, ids: &FxHashSet<KeyId>, keys: &mut Vec<Key>) {
    let mut found: Vec<&K> = ids
        .iter()
        .filter_map(|id| tracks.key(K::id_in(&id.lane)?, id.frame))
        .collect();
    found.sort_by(|one, two| (one.id(), one.frame()).cmp(&(two.id(), two.frame())));
    keys.extend(found.into_iter().cloned().map(K::into_key));
}

fn remove<K: Laned>(tracks: &mut Tracks<K>, ids: &FxHashSet<KeyId>) {
    for id in ids {
        if let Some(at) = K::id_in(&id.lane) {
            tracks.remove(at, id.frame);
        }
    }
}

fn laid<K: Laned>(keys: &[Key]) -> impl Iterator<Item = K> + '_ {
    keys.iter().filter_map(K::from_key).cloned()
}

macro_rules! keyed_tracks {
    ($($motion:ty;)+) => {
        $(impl Keyed for $motion {
            fn gather_keys(&self, ids: &FxHashSet<KeyId>) -> Vec<Key> {
                let mut keys = Vec::new();
                gather(self.tracks(), ids, &mut keys);
                keys
            }

            fn remove_keys(&mut self, ids: &FxHashSet<KeyId>) {
                remove(self.tracks_mut(), ids);
            }

            fn insert_keys(&mut self, keys: &[Key]) {
                let tracks = self.tracks_mut();
                for key in laid(keys) {
                    tracks.insert(key);
                }
            }
        })+
    };
}

keyed_tracks! {
    CameraMotion;
    Look;
    Emitter;
    WindMotion;
}

impl Keyed for Take {
    fn gather_keys(&self, ids: &FxHashSet<KeyId>) -> Vec<Key> {
        let mut keys = Vec::new();
        gather(&self.bones, ids, &mut keys);
        gather(&self.morphs, ids, &mut keys);
        if let Some(human) = &self.human {
            gather(human.parts(), ids, &mut keys);
        }
        keys
    }

    fn remove_keys(&mut self, ids: &FxHashSet<KeyId>) {
        remove(&mut self.bones, ids);
        remove(&mut self.morphs, ids);
        if let Some(human) = &mut self.human {
            for id in ids {
                if let Some(part) = PartKey::id_in(&id.lane) {
                    human.remove(*part, id.frame);
                }
            }
        }
    }

    fn insert_keys(&mut self, keys: &[Key]) {
        for key in laid(keys) {
            self.bones.insert(key);
        }
        for key in laid(keys) {
            self.morphs.insert(key);
        }
        for key in laid::<PartKey>(keys) {
            self.human.get_or_insert_default().insert(key);
        }
    }
}

impl<T: Keyed + Clone> Keyed for Arc<T> {
    fn gather_keys(&self, ids: &FxHashSet<KeyId>) -> Vec<Key> {
        self.as_ref().gather_keys(ids)
    }

    fn remove_keys(&mut self, ids: &FxHashSet<KeyId>) {
        Arc::make_mut(self).remove_keys(ids);
    }

    fn insert_keys(&mut self, keys: &[Key]) {
        Arc::make_mut(self).insert_keys(keys);
    }
}
