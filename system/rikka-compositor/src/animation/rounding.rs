use std::collections::HashMap;
use std::time::Instant;

use rikka_animation::{Curve, Motion, Property};
use rikka_compositor_desktop::WindowId;

use super::stepper::Stepper;
use super::track::Track;

const ROUNDING: Curve = Motion::Emphasized.curve(Property::Spatial);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Rounding {
    pub(crate) value: f32,
    pub(crate) settled: bool,
}

#[derive(Debug, Default)]
pub(crate) struct Roundings {
    tracks: HashMap<WindowId, Track>,
    stepper: Stepper,
}

impl Roundings {
    pub(crate) fn advance(&mut self, now: Instant) {
        let elapsed = self.stepper.step(now);
        for track in self.tracks.values_mut() {
            track.step(elapsed);
        }
    }

    pub(crate) fn follow(&mut self, window: WindowId, target: f32, animate: bool) -> Rounding {
        let track = self
            .tracks
            .entry(window)
            .or_insert_with(|| Track::new(target, ROUNDING));
        if track.target() != target {
            match animate {
                true => track.aim(target, ROUNDING),
                false => track.snap(target),
            }
        }
        Rounding {
            value: track.value(),
            settled: track.is_resting(),
        }
    }

    pub(crate) fn current(&self, window: WindowId) -> Option<f32> {
        self.tracks.get(&window).map(Track::value)
    }

    pub(crate) fn forget(&mut self, window: WindowId) {
        self.tracks.remove(&window);
    }
}
