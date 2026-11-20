use animate_motion::Tag;
use rikka_mmd::VmdCurve;
use rustc_hash::FxHashSet;

use crate::channel::Channel;
use crate::keyed::Keyed;
use crate::keys::{self, Key, KeyId};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Change {
    removed: Vec<Key>,
    added: Vec<Key>,
}

impl Change {
    #[must_use]
    pub fn laying(motion: &impl Keyed, keys: Vec<Key>) -> Self {
        let standing = motion.gather_keys(&keys::ids(&keys));
        let added: Vec<Key> = keys
            .into_iter()
            .filter(|key| !standing.contains(key))
            .collect();
        let replaced = keys::ids(&added);
        let removed = standing
            .into_iter()
            .filter(|key| replaced.contains(&key.id()))
            .collect();
        Self { removed, added }
    }

    #[must_use]
    pub fn deleting(motion: &impl Keyed, ids: &FxHashSet<KeyId>) -> Self {
        Self {
            removed: motion.gather_keys(ids),
            added: Vec::new(),
        }
    }

    #[must_use]
    pub fn shifting(motion: &impl Keyed, ids: &FxHashSet<KeyId>, by: i64) -> Self {
        let moving = motion.gather_keys(ids);
        let earliest = moving.iter().map(Key::frame).min().unwrap_or(0);
        let by = by.max(-i64::from(earliest));
        if by == 0 || moving.is_empty() {
            return Self::default();
        }
        let added: Vec<Key> = moving
            .iter()
            .cloned()
            .map(|key| {
                let frame = (i64::from(key.frame()) + by) as u32;
                key.with_frame(frame)
            })
            .collect();
        let mut covered = keys::ids(&added);
        covered.retain(|id| !ids.contains(id));
        let mut removed = moving;
        removed.extend(motion.gather_keys(&covered));
        Self { removed, added }
    }

    #[must_use]
    pub fn bending(removed: Vec<Key>, channels: &[Channel], curve: VmdCurve) -> Self {
        let added: Vec<Key> = removed
            .iter()
            .cloned()
            .map(|key| {
                channels
                    .iter()
                    .fold(key, |key, channel| channel.bent(key, curve))
            })
            .collect();
        if added == removed {
            return Self::default();
        }
        Self { removed, added }
    }

    #[must_use]
    pub fn tagging(keys: Vec<Key>, tag: Tag) -> Self {
        let removed: Vec<Key> = keys
            .into_iter()
            .filter(|key| matches!(key, Key::Part(key) if key.tag != tag || key.slopes.is_some()))
            .collect();
        let added = removed
            .iter()
            .cloned()
            .map(|key| match key {
                Key::Part(key) => Key::Part(key.retagged(tag)),
                key => key,
            })
            .collect();
        Self { removed, added }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.removed.is_empty() && self.added.is_empty()
    }

    #[must_use]
    pub fn added(&self) -> &[Key] {
        &self.added
    }

    pub fn apply(&self, motion: &mut impl Keyed) {
        swap(motion, &self.removed, &self.added);
    }

    pub fn revert(&self, motion: &mut impl Keyed) {
        swap(motion, &self.added, &self.removed);
    }
}

fn swap(motion: &mut impl Keyed, out: &[Key], into: &[Key]) {
    let mut removed = keys::ids(out);
    removed.extend(keys::ids(into));
    motion.remove_keys(&removed);
    motion.insert_keys(into);
}
