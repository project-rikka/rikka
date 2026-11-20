use std::time::{Duration, Instant};

use rikka_animation::{Curve, DurationToken, Easing, Motion, Property, Spring};
use rikka_compositor_geometry::{Logical, Point, Rect, Size};

use super::flight::Flight;
use super::stepper::Stepper;
use super::track::Track;
use crate::render::Visual;

const ENTRANCE: Curve = Curve::Spring(Spring::FAST_SPATIAL);

const REVEAL: Curve = Curve::Spring(Spring::DEFAULT_EFFECTS);

const DEPARTURE: Curve = Curve::Tween {
    easing: Easing::EmphasizedAccel,
    duration: DurationToken::Short4,
};

const RESHAPE: Curve = Motion::Emphasized.curve(Property::Spatial);

const RESTORE: Curve = Curve::Tween {
    easing: Easing::Emphasized,
    duration: DurationToken::Short4,
};

const ARRIVAL_SCALE: f32 = 0.86;

const DEPARTURE_SCALE: f32 = 0.90;

#[derive(Debug)]
enum Travel {
    Whole(Flight),
    Size([Track; 2]),
}

impl Travel {
    fn step(&mut self, elapsed: Duration) {
        match self {
            Travel::Whole(flight) => {
                flight.step(elapsed);
            }
            Travel::Size(tracks) => {
                for track in tracks {
                    track.step(elapsed);
                }
            }
        }
    }

    fn is_resting(&self) -> bool {
        match self {
            Travel::Whole(flight) => flight.is_resting(),
            Travel::Size(tracks) => tracks.iter().all(Track::is_resting),
        }
    }
}

#[derive(Debug)]
pub(crate) struct WindowMotion {
    scale: Track,
    alpha: Track,
    travel: Option<Travel>,
    stepper: Stepper,
}

impl WindowMotion {
    pub(crate) fn at_rest() -> Self {
        Self {
            scale: Track::new(1.0, ENTRANCE),
            alpha: Track::new(1.0, REVEAL),
            travel: None,
            stepper: Stepper::default(),
        }
    }

    pub(crate) fn open(&mut self, now: Instant) {
        self.advance(now);
        self.scale.snap(ARRIVAL_SCALE);
        self.alpha.snap(0.0);
        self.scale.aim(1.0, ENTRANCE);
        self.alpha.aim(1.0, REVEAL);
    }

    pub(crate) fn close(&mut self, now: Instant) {
        self.advance(now);
        self.scale.aim(DEPARTURE_SCALE, DEPARTURE);
        self.alpha.aim(0.0, DEPARTURE);
    }

    pub(crate) fn reshape(
        &mut self,
        from: Rect<f64, Logical>,
        to: Rect<i32, Logical>,
        launch: Option<f64>,
        now: Instant,
    ) {
        self.advance(now);
        if launch.is_none()
            && let Some(Travel::Whole(flight)) = self.travel.as_mut()
            && !flight.is_resting()
        {
            flight.aim(to.to_f64());
            return;
        }
        let mut flight = Flight::new(from, RESHAPE);
        if let Some(speed) = launch {
            flight.launch(Point::new(speed, 0.0));
        }
        flight.aim(to.to_f64());
        self.travel = Some(Travel::Whole(flight));
    }

    pub(crate) fn resize(
        &mut self,
        from: Size<i32, Logical>,
        to: Size<i32, Logical>,
        now: Instant,
    ) {
        self.advance(now);
        let mut tracks = extent(from).map(|value| Track::new(value, RESTORE));
        for (track, target) in tracks.iter_mut().zip(extent(to)) {
            track.aim(target, RESTORE);
        }
        self.travel = Some(Travel::Size(tracks));
    }

    pub(crate) fn stop_travel(&mut self) {
        self.travel = None;
    }

    pub(crate) fn advance(&mut self, now: Instant) {
        let elapsed = self.stepper.step(now);
        self.scale.step(elapsed);
        self.alpha.step(elapsed);
        if let Some(travel) = self.travel.as_mut() {
            travel.step(elapsed);
        }
    }

    pub(crate) fn is_animating(&self) -> bool {
        !self.scale.is_resting()
            || !self.alpha.is_resting()
            || self
                .travel
                .as_ref()
                .is_some_and(|travel| !travel.is_resting())
    }

    pub(crate) fn sample(&mut self, now: Instant, geometry: Rect<i32, Logical>) -> Option<Visual> {
        self.advance(now);
        let reshaped = self.reshaped(geometry);
        let scale = f64::from(self.scale.value().max(0.0));
        let alpha = self.alpha.value().clamp(0.0, 1.0);
        if reshaped.is_none() && scale == 1.0 && alpha == 1.0 {
            return None;
        }
        let rect = reshaped.unwrap_or_else(|| geometry.to_f64());
        Some(Visual {
            rect: shrunk_about_center(rect, scale),
            alpha,
        })
    }

    fn reshaped(&mut self, geometry: Rect<i32, Logical>) -> Option<Rect<f64, Logical>> {
        let travel = self.travel.as_mut()?;
        if let Travel::Whole(flight) = travel {
            flight.aim(geometry.to_f64());
        }
        if travel.is_resting() {
            self.travel = None;
            return None;
        }
        Some(match travel {
            Travel::Whole(flight) => flight.rect(),
            Travel::Size([w, h]) => Rect::new(
                geometry.loc.to_f64(),
                Size::new(f64::from(w.value()), f64::from(h.value())),
            ),
        })
    }
}

fn shrunk_about_center(rect: Rect<f64, Logical>, scale: f64) -> Rect<f64, Logical> {
    let size = Size::new(rect.size.w * scale, rect.size.h * scale);
    let loc = rect.loc + Point::new((rect.size.w - size.w) / 2.0, (rect.size.h - size.h) / 2.0);
    Rect::new(loc, size)
}

fn extent(size: Size<i32, Logical>) -> [f32; 2] {
    [size.w as f32, size.h as f32]
}
