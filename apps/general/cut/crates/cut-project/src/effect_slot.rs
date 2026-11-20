use std::collections::BTreeMap;
use std::ops::Range;

use rikka_effects::Effect;

use crate::animation::{Scalar, Track};

const UNCHANGED: f32 = 1e-4;

#[derive(Clone, Debug, PartialEq)]
pub struct EffectSlot {
    pub effect: Effect,
    pub enabled: bool,
    pub tracks: BTreeMap<String, Track<Scalar>>,
}

impl EffectSlot {
    #[must_use]
    pub fn new(effect: impl Into<Effect>) -> EffectSlot {
        EffectSlot {
            effect: effect.into(),
            enabled: true,
            tracks: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn track(&self, knob: &str) -> Option<&Track<Scalar>> {
        self.tracks.get(knob).filter(|track| !track.is_empty())
    }

    #[must_use]
    pub fn is_animated(&self) -> bool {
        self.tracks.values().any(|track| !track.is_empty())
    }

    #[must_use]
    pub fn effect_at(&self, at: f64) -> Effect {
        let mut effect = self.effect.clone();
        for (knob, track) in &self.tracks {
            if let Some(value) = track.sample(at) {
                effect.set(knob, value);
            }
        }
        effect
    }

    pub(crate) fn tuned(&mut self, target: Effect, at: i64) {
        if target.kind() != self.effect.kind() {
            self.effect = target;
            self.tracks.clear();
            return;
        }
        let current = self.effect_at(at as f64);
        let mut adjusted = target.clone();
        for (knob, track) in &mut self.tracks {
            if track.is_empty() {
                continue;
            }
            if let Some(base) = self.effect.value(knob) {
                adjusted.set(knob, base);
            }
            let (Some(before), Some(after)) = (current.value(knob), target.value(knob)) else {
                continue;
            };
            if (after - before).abs() > UNCHANGED {
                track.set(at, after);
            }
        }
        self.effect = adjusted;
    }

    pub(crate) fn cropped(&self, span: &Range<i64>) -> EffectSlot {
        EffectSlot {
            effect: self.effect.clone(),
            enabled: self.enabled,
            tracks: self
                .tracks
                .iter()
                .map(|(knob, track)| (knob.clone(), track.cropped(span)))
                .filter(|(_, track)| !track.is_empty())
                .collect(),
        }
    }

    pub(crate) fn checked(self, length: i64) -> EffectSlot {
        let effect = self.effect.checked();
        let knobs = effect.knobs();
        let tracks = self
            .tracks
            .into_iter()
            .filter_map(|(key, mut track)| {
                let knob = knobs.iter().find(|knob| knob.key == key)?;
                track.map_values(|value| knob.scale.clamp(value));
                Some((key, track))
            })
            .collect();
        EffectSlot {
            effect,
            enabled: self.enabled,
            tracks,
        }
        .cropped(&(0..length))
    }
}

#[must_use]
pub fn sampled(slots: &[EffectSlot], at: f64) -> Vec<Effect> {
    slots
        .iter()
        .filter(|slot| slot.enabled)
        .map(|slot| slot.effect_at(at))
        .collect()
}
