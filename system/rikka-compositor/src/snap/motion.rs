use std::time::Duration;

use rikka_animation::{Curve, DurationToken, Easing, Motion, Property};
use rikka_compositor_geometry::{Logical, Rect};

use crate::animation::{Flight, Track};

const ARRIVAL: Curve = Curve::Tween {
    easing: Easing::Emphasized,
    duration: DurationToken::Short4,
};

const DEPARTURE: Curve = Curve::Tween {
    easing: Easing::EmphasizedAccel,
    duration: DurationToken::Short3,
};

pub(crate) const TRAVEL: Curve = Motion::Emphasized.curve(Property::Spatial);

#[derive(Clone, Copy, Debug)]
pub(crate) struct Fade {
    track: Track,
    leaving: bool,
}

impl Fade {
    pub(crate) fn arriving() -> Self {
        let mut track = Track::new(0.0, ARRIVAL);
        track.aim(1.0, ARRIVAL);
        Self {
            track,
            leaving: false,
        }
    }

    pub(crate) fn hidden() -> Self {
        Self {
            track: Track::new(0.0, ARRIVAL),
            leaving: true,
        }
    }

    pub(crate) fn show(&mut self) {
        if !self.leaving {
            return;
        }
        self.leaving = false;
        self.track.aim(1.0, ARRIVAL);
    }

    pub(crate) fn leave(&mut self) {
        if self.leaving {
            return;
        }
        self.leaving = true;
        self.track.aim(0.0, DEPARTURE);
    }

    pub(crate) fn value(&self) -> f32 {
        self.track.value().clamp(0.0, 1.0)
    }

    pub(crate) fn is_leaving(&self) -> bool {
        self.leaving
    }

    pub(crate) fn is_gone(&self) -> bool {
        self.leaving && self.track.is_resting()
    }

    pub(crate) fn step(&mut self, elapsed: Duration) -> bool {
        self.track.step(elapsed);
        !self.track.is_resting()
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Spread {
    track: Track,
}

impl Spread {
    pub(crate) fn shut() -> Self {
        Self {
            track: Track::new(0.0, ARRIVAL),
        }
    }

    pub(crate) fn aim(&mut self, open: bool) {
        if open {
            self.track.aim(1.0, ARRIVAL);
        } else {
            self.track.aim(0.0, DEPARTURE);
        }
    }

    pub(crate) fn value(&self) -> f32 {
        self.track.value().clamp(0.0, 1.0)
    }

    pub(crate) fn step(&mut self, elapsed: Duration) -> bool {
        self.track.step(elapsed);
        !self.track.is_resting()
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Shape {
    flight: Flight,
    fade: Fade,
}

impl Shape {
    pub(crate) fn nowhere() -> Self {
        Self {
            flight: Flight::new(Rect::default(), TRAVEL),
            fade: Fade::hidden(),
        }
    }

    pub(crate) fn aim(&mut self, at: Option<Rect<i32, Logical>>) {
        let Some(at) = at else {
            self.fade.leave();
            return;
        };
        if self.fade.is_gone() {
            self.flight.jump(at.to_f64());
        } else {
            self.flight.aim(at.to_f64());
        }
        self.fade.show();
    }

    pub(crate) fn step(&mut self, elapsed: Duration) -> bool {
        self.fade.step(elapsed) | self.flight.step(elapsed)
    }

    pub(crate) fn sample(&self) -> Option<(Rect<f64, Logical>, f32)> {
        let alpha = self.fade.value();
        if alpha <= 0.0 {
            return None;
        }
        let rect = self.flight.rect();
        if rect.size.w <= 0.0 || rect.size.h <= 0.0 {
            return None;
        }
        Some((rect, alpha))
    }
}
