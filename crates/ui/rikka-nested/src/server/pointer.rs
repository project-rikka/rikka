use std::time::{Duration, Instant};

use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_seat::{Focus, Motion, RelativeMotion};

use super::state::State;
use crate::view::ViewId;

#[derive(Debug, Default)]
pub(crate) struct Pointer {
    pub(crate) view: Option<ViewId>,
    pub(crate) at: Option<Point<f64, Logical>>,
}

impl State {
    pub(crate) fn input_time(&self, at: Instant) -> Duration {
        let now = Instant::now();
        let since = now.saturating_duration_since(at);
        rikka_compositor_reactor::Timestamp::now()
            .saturating_sub(since)
            .as_monotonic()
    }

    pub(crate) fn surface_at(&self, view: ViewId, at: Point<f64, Logical>) -> Option<Focus> {
        let roots = self.roots(view);
        for (root, origin) in roots.iter().rev() {
            let local = at - origin.to_f64();
            if let Some((surface, within)) = self.surfaces.surface_under(*root, local) {
                return Some(Focus::new(surface, at - within));
            }
        }
        None
    }

    pub(crate) fn point_to(&mut self, view: ViewId, at: Point<f64, Logical>, time: Duration) {
        let under = self.surface_at(view, at);
        self.pointer.view = Some(view);
        self.pointer.at = Some(at);
        let seat = self.seat;
        let mut seat_handle = rikka_compositor_seat::seat(self, seat);
        seat_handle.motion(Motion {
            location: at,
            under,
            time,
        });
    }

    pub(crate) fn pointer_left(&mut self, view: ViewId, time: Duration) {
        if self.pointer.view != Some(view) {
            return;
        }
        self.pointer.view = None;
        let location = self.pointer.at.take().unwrap_or_default();
        let seat = self.seat;
        let mut seat_handle = rikka_compositor_seat::seat(self, seat);
        seat_handle.motion(Motion {
            location,
            under: None,
            time,
        });
        seat_handle.frame();
    }

    pub(crate) fn travel(&mut self, delta: Point<f64, Logical>, time: Duration) {
        let seat = self.seat;
        let mut seat_handle = rikka_compositor_seat::seat(self, seat);
        seat_handle.relative_motion(RelativeMotion {
            delta,
            unaccelerated: delta,
            time,
        });
        seat_handle.frame();
    }
}
