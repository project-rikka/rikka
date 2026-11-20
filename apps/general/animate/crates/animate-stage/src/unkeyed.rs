use std::collections::BTreeMap;

use animate_fluid::{EmitterId, EmitterSetting, EmitterValue};
use animate_wind::WindSetting;

use crate::wind::Blowing;

#[derive(Clone, Debug, PartialEq)]
pub struct UnkeyedValues<O, S, V> {
    owner: O,
    frame: u32,
    values: BTreeMap<S, V>,
}

impl<O: Copy + PartialEq, S: Copy + Ord, V: Copy> UnkeyedValues<O, S, V> {
    fn matches(&self, owner: O, frame: u32) -> bool {
        self.owner == owner && self.frame == frame
    }

    pub fn get(&self, setting: S) -> Option<V> {
        self.values.get(&setting).copied()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn apply(&self, mut set: impl FnMut(S, V)) {
        for (setting, value) in &self.values {
            set(*setting, *value);
        }
    }

    pub fn into_values(self) -> BTreeMap<S, V> {
        self.values
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Unkeyed<O, S, V> {
    slot: Option<UnkeyedValues<O, S, V>>,
}

impl<O, S, V> Default for Unkeyed<O, S, V> {
    fn default() -> Self {
        Self { slot: None }
    }
}

impl<O: Copy + PartialEq, S: Copy + Ord, V: Copy> Unkeyed<O, S, V> {
    pub fn at(&self, owner: O, frame: u32) -> Option<&UnkeyedValues<O, S, V>> {
        self.slot
            .as_ref()
            .filter(|unkeyed| unkeyed.matches(owner, frame))
    }

    pub fn owner(&self) -> Option<O> {
        self.slot.as_ref().map(|unkeyed| unkeyed.owner)
    }

    pub fn is_stale(&self, running: bool, frame: u32) -> bool {
        self.slot
            .as_ref()
            .is_some_and(|unkeyed| running || unkeyed.frame != frame)
    }

    pub fn set(&mut self, owner: O, frame: u32, setting: S, value: V) {
        let unkeyed = match &mut self.slot {
            Some(unkeyed) if unkeyed.matches(owner, frame) => unkeyed,
            slot => slot.insert(UnkeyedValues {
                owner,
                frame,
                values: BTreeMap::new(),
            }),
        };
        unkeyed.values.insert(setting, value);
    }

    pub fn clear(&mut self, owner: O, setting: S) -> bool {
        let Some(unkeyed) = self.slot.as_mut().filter(|unkeyed| unkeyed.owner == owner) else {
            return false;
        };
        if unkeyed.values.remove(&setting).is_none() {
            return false;
        }
        if unkeyed.values.is_empty() {
            self.slot = None;
        }
        true
    }

    pub fn discard(&mut self) -> bool {
        self.slot.take().is_some()
    }

    pub fn take_at(&mut self, owner: O, frame: u32) -> Option<Option<UnkeyedValues<O, S, V>>> {
        let unkeyed = self.slot.take()?;
        Some(unkeyed.matches(owner, frame).then_some(unkeyed))
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drafts {
    pub pour: Unkeyed<EmitterId, EmitterSetting, EmitterValue>,
    pub wind: Unkeyed<Blowing, WindSetting, f32>,
}

impl Drafts {
    pub fn is_stale(&self, running: bool, frame: u32) -> bool {
        self.pour.is_stale(running, frame) || self.wind.is_stale(running, frame)
    }

    pub fn discard_stale(&mut self, running: bool, frame: u32) {
        if self.pour.is_stale(running, frame) {
            self.pour.discard();
        }
        if self.wind.is_stale(running, frame) {
            self.wind.discard();
        }
    }
}
