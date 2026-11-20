use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_input::PointerRestrictions;
use rikka_compositor_seat::Motion;

use crate::context::{InputCtx, WindowsCtx};
use crate::context::{input_ref, person};
use crate::render::monotonic;

impl WindowsCtx<'_> {
    pub(crate) fn pointer_moved_under(&mut self) {
        let location = self.cursor.position();
        self.follow_pointer(location);
        self.pointer_follows_surfaces();
    }

    pub(crate) fn pointer_follows_surfaces(&mut self) {
        let seat = self.input.seat;
        let Some(pointer) = self.seats.get(seat).map(|seat_state| seat_state.pointer()) else {
            return;
        };
        if pointer.grabbed() || !pointer.buttons().is_empty() {
            return;
        }
        let (location, focus) = (pointer.location(), pointer.focus());
        let under = input_ref!(self).focus_at(location);
        if under == focus {
            return;
        }
        person!(self).warp(Motion {
            location,
            under,
            time: monotonic(),
        });
    }
}

impl InputCtx<'_> {
    pub(super) fn warp_pointer(&mut self, to: Point<f64, Logical>) {
        let to = self
            .outputs
            .route(false, PointerRestrictions::empty())
            .clamp(to);
        let to = person!(self).constrain(to);
        self.cursor_to(to);
        let under = input_ref!(self).focus_at(to);
        person!(self).warp(Motion {
            location: to,
            under,
            time: monotonic(),
        });
    }
}
