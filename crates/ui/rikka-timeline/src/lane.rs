use rikka_keyframe::Frame;
use rikka_types::Rect;

use crate::scale::TimeScale;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimeLane {
    pub area: Rect,
    pub origin: f32,
    pub scale: TimeScale,
}

impl TimeLane {
    #[must_use]
    pub fn x_of(&self, frame: f64) -> f32 {
        self.origin + self.scale.x_of(frame)
    }

    #[must_use]
    pub fn frame_at(&self, x: f32) -> f64 {
        self.scale.frame_at(x - self.origin)
    }

    #[must_use]
    pub fn frames_shown(&self) -> (f64, f64) {
        (
            self.frame_at(self.area.min_x()),
            self.frame_at(self.area.max_x()),
        )
    }

    #[must_use]
    pub fn nearest<'k, K, F: Frame>(
        &self,
        keys: &'k [K],
        frame: impl Fn(&K) -> F,
        x: f32,
        reach: f32,
    ) -> Option<&'k K> {
        self.scale.nearest(keys, frame, x - self.origin, reach)
    }
}
