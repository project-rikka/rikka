mod deliver;
mod events;
mod grab;

use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_protocols::wl_touch::{Request, WlTouch};
use rikka_compositor_server::ByClient;
use rikka_wayland_wire::ClientId;
use smallvec::SmallVec;

use crate::access::Access;
use crate::focus::Focus;
use crate::grab_slot::GrabSlot;
use crate::ids::SeatId;

pub(crate) use deliver::{
    touch_cancel, touch_down, touch_frame, touch_motion, touch_shape, touch_surface_gone, touch_up,
};
pub use events::{TouchDown, TouchMotion, TouchShape, TouchUp};
pub use grab::{TouchGrab, TouchHandle};
pub(crate) use grab::{dispatch, end_grab, set_grab};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchGrabStart {
    pub serial: u32,
    pub slot: u32,
    pub location: Point<f64, Logical>,
    pub focus: Focus,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TouchPoint {
    pub(crate) slot: u32,
    pub(crate) focus: Focus,
    pub(crate) client: ClientId,
    pub(crate) serial: u32,
    pub(crate) location: Point<f64, Logical>,
}

#[derive(Debug, Default)]
pub struct Touch {
    pub(crate) touches: ByClient<WlTouch>,
    pub(crate) points: Vec<TouchPoint>,
    pub(crate) unframed: SmallVec<[ClientId; 2]>,
    pub(crate) grab: GrabSlot,
}

impl Touch {
    pub fn active(&self) -> bool {
        !self.points.is_empty()
    }

    pub fn focus(&self, slot: u32) -> Option<Focus> {
        self.points
            .iter()
            .find(|point| point.slot == slot)
            .map(|point| point.focus)
    }

    pub fn grabbed(&self) -> bool {
        self.grab.grabbed()
    }

    pub fn grab_start(&self, serial: u32) -> Option<TouchGrabStart> {
        if self.grab.grabbed() {
            return None;
        }
        self.points
            .iter()
            .find(|point| point.serial == serial)
            .map(|point| TouchGrabStart {
                serial,
                slot: point.slot,
                location: point.location,
                focus: point.focus,
            })
    }

    pub(crate) fn mark(&mut self, client: ClientId) {
        if !self.unframed.contains(&client) {
            self.unframed.push(client);
        }
    }
}

pub(crate) fn touch_request<A: Access>(_state: &mut A, _touch: WlTouch, _request: Request) {}

pub(crate) fn touch_destroyed<A: Access>(state: &mut A, touch: WlTouch) {
    let Some(seat) = touch.key::<SeatId>() else {
        return;
    };
    if let Some(entry) = state.seats().seat_mut(seat) {
        entry.touch.touches.remove(&touch);
    }
}
