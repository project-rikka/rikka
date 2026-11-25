mod constraint;
mod cursor;
mod deliver;
mod events;
mod gestures;
mod grab;
mod protocol;
mod relative;
mod shape;
mod shape_device;
mod warp;

use std::fmt;

use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_protocols::wl_pointer::WlPointer;
use rikka_compositor_protocols::zwp_pointer_gesture_hold_v1::ZwpPointerGestureHoldV1;
use rikka_compositor_protocols::zwp_pointer_gesture_pinch_v1::ZwpPointerGesturePinchV1;
use rikka_compositor_protocols::zwp_pointer_gesture_swipe_v1::ZwpPointerGestureSwipeV1;
use rikka_compositor_protocols::zwp_relative_pointer_v1::ZwpRelativePointerV1;
use rikka_compositor_server::ByClient;
use rikka_compositor_surface::SurfaceId;
use rikka_wayland_wire::ClientId;
use smallvec::SmallVec;

use crate::focus::{Focus, Focused};
use crate::grab_slot::GrabSlot;
use crate::ids::SeatId;

pub(crate) use constraint::{
    Constraint, Handle as ConstraintHandle, confined_request, constrain, constraint_destroyed,
    constraints_committed, constraints_manager, constraints_surface_gone, locked_request,
};
pub use constraint::{ConstraintKind, ConstraintState};
pub use cursor::CursorImage;
pub(crate) use cursor::{CursorOwner, cursor_surface_gone, may_set_cursor, set_cursor, show};
pub(crate) use deliver::{
    conceal, deliver_axis, deliver_button, deliver_frame, deliver_gesture, deliver_motion,
    deliver_relative, deliver_warp, introduce, pointer_surface_gone, reveal,
};
pub use events::{Axis, AxisMotion, Button, Motion, RelativeMotion};
pub(crate) use gestures::{gesture_destroyed, gesture_request, gestures_manager};
pub use grab::{GrabFocus, PointerGrab, PointerHandle};
pub(crate) use grab::{dispatch, end_grab, set_grab};
pub(crate) use protocol::{pointer_destroyed, pointer_request};
pub(crate) use relative::{relative_destroyed, relative_manager, relative_request};
pub use shape::CursorShape;
pub(crate) use shape_device::{shape_device, shape_manager};
pub(crate) use warp::warp_request;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GrabStart {
    pub serial: u32,
    pub button: u32,
    pub location: Point<f64, Logical>,
    pub focus: Option<Focus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GestureKind {
    Swipe,
    Pinch,
    Hold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ActiveGesture {
    pub(crate) kind: GestureKind,
    pub(crate) surface: SurfaceId,
    pub(crate) client: ClientId,
}

pub struct Pointer {
    pub(crate) pointers: ByClient<WlPointer>,
    pub(crate) relative: ByClient<ZwpRelativePointerV1>,
    pub(crate) swipes: ByClient<ZwpPointerGestureSwipeV1>,
    pub(crate) pinches: ByClient<ZwpPointerGesturePinchV1>,
    pub(crate) holds: ByClient<ZwpPointerGestureHoldV1>,
    pub(crate) location: Point<f64, Logical>,
    pub(crate) focused: Option<Focused>,
    pub(crate) under: Option<Focus>,
    pub(crate) enter_serial: u32,
    pub(crate) buttons: Vec<u32>,
    pub(crate) press: Option<GrabStart>,
    pub(crate) grab: GrabSlot,
    pub(crate) cursor: CursorImage,
    pub(crate) constraints: Vec<Constraint>,
    pub(crate) gesture: Option<ActiveGesture>,
    pub(crate) unframed: SmallVec<[(SeatId, ClientId); 2]>,
}

impl Pointer {
    pub(crate) fn new() -> Self {
        Self {
            pointers: ByClient::default(),
            relative: ByClient::default(),
            swipes: ByClient::default(),
            pinches: ByClient::default(),
            holds: ByClient::default(),
            location: Point::zero(),
            focused: None,
            under: None,
            enter_serial: 0,
            buttons: Vec::new(),
            press: None,
            grab: GrabSlot::default(),
            cursor: CursorImage::default(),
            constraints: Vec::new(),
            gesture: None,
            unframed: SmallVec::new(),
        }
    }

    pub fn location(&self) -> Point<f64, Logical> {
        self.location
    }

    pub fn focus(&self) -> Option<Focus> {
        self.focused.as_ref().map(|focused| focused.focus)
    }

    pub fn buttons(&self) -> &[u32] {
        &self.buttons
    }

    pub fn grabbed(&self) -> bool {
        self.grab.grabbed()
    }

    pub fn cursor(&self) -> CursorImage {
        self.cursor
    }

    pub fn grab_start(&self, serial: u32) -> Option<GrabStart> {
        if self.grab.grabbed() || self.buttons.is_empty() {
            return None;
        }
        self.press.filter(|press| press.serial == serial)
    }

    pub fn constraint(&self) -> Option<(SurfaceId, ConstraintKind)> {
        self.constraints
            .iter()
            .find(|constraint| constraint.active)
            .map(|constraint| (constraint.surface, constraint.kind))
    }

    pub(crate) fn constrains(&self, surface: SurfaceId) -> bool {
        self.constraints
            .iter()
            .any(|constraint| constraint.surface == surface)
    }

    pub(crate) fn focused_surface(&self) -> Option<SurfaceId> {
        self.focused.as_ref().map(|focused| focused.focus.surface)
    }

    pub(crate) fn mark(&mut self, wire: SeatId, client: ClientId) {
        if !self.unframed.contains(&(wire, client)) {
            self.unframed.push((wire, client));
        }
    }
}

impl fmt::Debug for Pointer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Pointer")
            .field("location", &self.location)
            .field("focus", &self.focus())
            .field("buttons", &self.buttons)
            .field("grab", &self.grab)
            .field("cursor", &self.cursor)
            .field("constraints", &self.constraints)
            .finish_non_exhaustive()
    }
}
