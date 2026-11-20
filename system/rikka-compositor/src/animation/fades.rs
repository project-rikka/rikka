use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

use rikka_animation::{Curve, Presence};

use super::stepper::Stepper;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Fade<K> {
    pub(crate) key: K,
    pub(crate) opacity: f32,
    pub(crate) settled: bool,
}

#[derive(Clone, Copy, Debug)]
struct Fading {
    presence: Presence,
    shown: bool,
}

#[derive(Debug)]
pub(crate) struct Fades<K> {
    curve: Curve,
    fading: HashMap<K, Fading>,
    stepper: Stepper,
}

impl<K: Copy + Eq + Hash> Fades<K> {
    pub(crate) fn new(curve: Curve) -> Self {
        Self {
            curve,
            fading: HashMap::new(),
            stepper: Stepper::default(),
        }
    }

    pub(crate) fn advance(&mut self, now: Instant) {
        let elapsed = self.stepper.step(now);
        self.step(elapsed);
    }

    pub(crate) fn follow(&mut self, wanted: impl IntoIterator<Item = (K, bool)>, animate: bool) {
        for fading in self.fading.values_mut() {
            fading.shown = false;
        }
        let mut listed = Vec::new();
        for (key, shown) in wanted {
            listed.push(key);
            self.fading
                .entry(key)
                .or_insert(Fading {
                    presence: Presence::HIDDEN,
                    shown: false,
                })
                .shown = shown;
        }
        match animate {
            true => self.step(Duration::ZERO),
            false => {
                for fading in self.fading.values_mut() {
                    fading.presence = match fading.shown {
                        true => Presence::SHOWN,
                        false => Presence::HIDDEN,
                    };
                }
            }
        }
        self.fading.retain(|key, fading| {
            listed.contains(key)
                || !fading.presence.is_resting()
                || fading.presence.progress() > 0.0
        });
    }

    pub(crate) fn forget(&mut self, key: K) {
        self.fading.remove(&key);
    }

    pub(crate) fn fades(&self) -> impl Iterator<Item = Fade<K>> + '_ {
        self.fading.iter().map(|(key, fading)| Fade {
            key: *key,
            opacity: fading.presence.progress(),
            settled: fading.presence.is_resting(),
        })
    }

    fn step(&mut self, elapsed: Duration) {
        for fading in self.fading.values_mut() {
            fading.presence.advance(fading.shown, self.curve, elapsed);
        }
    }
}
