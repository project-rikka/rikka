use std::time::Duration;

use rikka_animation::Curve;
use rikka_compositor_geometry::{Logical, Point, Rect, Size};

use super::track::Track;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Flight {
    tracks: [Track; 4],
    curve: Curve,
}

impl Flight {
    pub(crate) fn new(rect: Rect<f64, Logical>, curve: Curve) -> Self {
        Self {
            tracks: corners(rect).map(|value| Track::new(value, curve)),
            curve,
        }
    }

    pub(crate) fn aim(&mut self, at: Rect<f64, Logical>) {
        for (track, value) in self.tracks.iter_mut().zip(corners(at)) {
            track.aim(value, self.curve);
        }
    }

    pub(crate) fn jump(&mut self, at: Rect<f64, Logical>) {
        for (track, value) in self.tracks.iter_mut().zip(corners(at)) {
            track.snap(value);
        }
    }

    pub(crate) fn launch(&mut self, velocity: Point<f64, Logical>) {
        let [x, y, ..] = &mut self.tracks;
        x.launch(velocity.x as f32);
        y.launch(velocity.y as f32);
    }

    pub(crate) fn rect(&self) -> Rect<f64, Logical> {
        let [x, y, w, h] = self.tracks.map(|track| f64::from(track.value()));
        Rect::new(Point::new(x, y), Size::new(w, h))
    }

    pub(crate) fn is_resting(&self) -> bool {
        self.tracks.iter().all(Track::is_resting)
    }

    pub(crate) fn step(&mut self, elapsed: Duration) -> bool {
        for track in &mut self.tracks {
            track.step(elapsed);
        }
        !self.is_resting()
    }
}

fn corners(rect: Rect<f64, Logical>) -> [f32; 4] {
    [
        rect.loc.x as f32,
        rect.loc.y as f32,
        rect.size.w as f32,
        rect.size.h as f32,
    ]
}
