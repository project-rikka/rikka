use rikka_compositor_desktop::WindowId;
use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_protocols::wl_seat::WlSeat;
use rikka_compositor_seat::SeatId;
use rikka_compositor_xwm::XWindowId;

use crate::context::WindowsCtx;
use crate::context::{InputRef, input_ref};
use rikka_compositor_tiling::Edges;

impl InputRef<'_> {
    fn pressed_for(&self, seat: &WlSeat, serial: u32) -> Option<Point<f64, Logical>> {
        if seat.key::<SeatId>() != Some(self.input.seat) || self.windows.desktop.locked() {
            return None;
        }
        let press = self
            .seats
            .get(self.input.seat)?
            .pointer()
            .grab_start(serial);
        if press.is_none() {
            tracing::debug!("ignoring move request without a pressed button");
        }
        Some(press?.location)
    }

    fn press_in_force(&self) -> Option<Point<f64, Logical>> {
        let pointer = self.seats.get(self.input.seat)?.pointer();
        if pointer.grabbed() || pointer.buttons().is_empty() || self.windows.desktop.locked() {
            tracing::debug!("ignoring X11 move request without a pressed button");
            return None;
        }
        Some(pointer.location())
    }
}

impl WindowsCtx<'_> {
    pub(crate) fn move_requested(&mut self, window: WindowId, seat: &WlSeat, serial: u32) {
        if let Some(from) = input_ref!(self).pressed_for(seat, serial) {
            self.start_move(window, from);
        }
    }

    pub(crate) fn resize_requested(
        &mut self,
        window: WindowId,
        seat: &WlSeat,
        serial: u32,
        edges: Edges,
    ) {
        if let Some(from) = input_ref!(self).pressed_for(seat, serial) {
            self.start_resize(window, edges, from);
        }
    }

    pub(crate) fn x11_move_requested(&mut self, x11: XWindowId) {
        if let Some(window) = self.windows.desktop.window_for_x11(x11)
            && let Some(from) = input_ref!(self).press_in_force()
        {
            self.start_move(window, from);
        }
    }

    pub(crate) fn x11_resize_requested(&mut self, x11: XWindowId, edges: Edges) {
        if let Some(window) = self.windows.desktop.window_for_x11(x11)
            && let Some(from) = input_ref!(self).press_in_force()
        {
            self.start_resize(window, edges, from);
        }
    }
}
