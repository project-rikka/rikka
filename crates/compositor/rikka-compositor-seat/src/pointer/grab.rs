use std::fmt;

use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_types::Gesture;

use crate::access::Access;
use crate::carry::Claim;
use crate::event::SeatEvent;
use crate::grab_slot::GrabOutcome;
use crate::host::SeatHost;
use crate::ids::SeatId;
use crate::pointer::deliver::{
    change_focus, deliver_axis, deliver_button, deliver_frame, deliver_gesture, deliver_motion,
    deliver_relative, refocus,
};
use crate::pointer::events::{Axis, Button, Motion, RelativeMotion};

pub trait PointerGrab<S: SeatHost>: fmt::Debug + 'static {
    fn motion(&mut self, pointer: &mut PointerHandle<'_, S>, motion: &Motion) {
        pointer.motion(motion);
    }

    fn relative_motion(&mut self, pointer: &mut PointerHandle<'_, S>, motion: &RelativeMotion) {
        pointer.relative_motion(motion);
    }

    fn button(&mut self, pointer: &mut PointerHandle<'_, S>, button: &Button) {
        pointer.button(button);
    }

    fn axis(&mut self, pointer: &mut PointerHandle<'_, S>, axis: &Axis) {
        pointer.axis(axis);
    }

    fn gesture(&mut self, pointer: &mut PointerHandle<'_, S>, gesture: &Gesture) {
        pointer.gesture(gesture);
    }

    fn frame(&mut self, pointer: &mut PointerHandle<'_, S>) {
        pointer.frame();
    }

    fn ended(&mut self, _state: &mut S) {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrabFocus {
    Keep,
    Clear,
}

pub struct PointerHandle<'a, S: SeatHost> {
    state: &'a mut S,
    seat: SeatId,
    serial: u32,
}

impl<S: SeatHost> PointerHandle<'_, S> {
    pub fn state(&mut self) -> &mut S {
        self.state
    }

    pub fn seat(&self) -> SeatId {
        self.seat
    }

    pub fn serial(&self) -> u32 {
        self.serial
    }

    pub fn location(&mut self) -> Point<f64, Logical> {
        self.state
            .seats()
            .get(self.seat)
            .map(|entry| entry.pointer.location)
            .unwrap_or_else(Point::zero)
    }

    pub fn pressed(&mut self) -> bool {
        self.state
            .seats()
            .get(self.seat)
            .is_some_and(|entry| !entry.pointer.buttons.is_empty())
    }

    pub fn motion(&mut self, motion: &Motion) {
        deliver_motion(self.state, self.seat, motion);
    }

    pub fn relative_motion(&mut self, motion: &RelativeMotion) {
        deliver_relative(self.state, self.seat, motion);
    }

    pub fn button(&mut self, button: &Button) {
        deliver_button(self.state, self.seat, button, self.serial);
    }

    pub fn axis(&mut self, axis: &Axis) {
        deliver_axis(self.state, self.seat, axis);
    }

    pub fn gesture(&mut self, gesture: &Gesture) {
        deliver_gesture(self.state, self.seat, gesture, self.serial);
    }

    pub fn frame(&mut self) {
        deliver_frame(self.state, self.seat);
    }

    pub fn end_grab(&mut self) {
        end_grab::<S>(self.state, self.seat);
    }
}

impl<S: SeatHost> fmt::Debug for PointerHandle<'_, S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PointerHandle")
            .field("seat", &self.seat)
            .field("serial", &self.serial)
            .finish_non_exhaustive()
    }
}

pub(crate) fn set_grab<A: Access>(
    state: &mut A,
    seat: SeatId,
    grab: impl PointerGrab<A::Host>,
    focus: GrabFocus,
) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let erased: Box<dyn PointerGrab<A::Host>> = Box::new(grab);
    if let Some(previous) = entry.pointer.grab.install(erased) {
        state.pointer_grab_ended(previous);
    }
    if focus == GrabFocus::Clear {
        change_focus(state, seat, None, Claim::Passive);
        deliver_frame(state, seat);
    }
    state.report(SeatEvent::PointerGrab {
        seat,
        grabbed: true,
    });
}

pub(crate) fn end_grab<A: Access>(state: &mut A, seat: SeatId) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    if let Some(Some(grab)) = entry.pointer.grab.release::<dyn PointerGrab<A::Host>>() {
        state.pointer_grab_ended(grab);
        finish(state, seat);
    }
}

fn finish<A: Access>(state: &mut A, seat: SeatId) {
    state.report(SeatEvent::PointerGrab {
        seat,
        grabbed: false,
    });
    refocus(state, seat);
    deliver_frame(state, seat);
}

pub(crate) fn dispatch<S: SeatHost>(
    state: &mut S,
    seat: SeatId,
    serial: u32,
    call: impl FnOnce(&mut dyn PointerGrab<S>, &mut PointerHandle<'_, S>),
) -> bool {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return true;
    };
    let Some(mut taken) = entry.pointer.grab.take::<dyn PointerGrab<S>>() else {
        return false;
    };
    let mut handle = PointerHandle {
        state: &mut *state,
        seat,
        serial,
    };
    call(taken.grab(), &mut handle);
    let Some(entry) = state.seats().seat_mut(seat) else {
        return true;
    };
    match entry.pointer.grab.restore(taken) {
        GrabOutcome::Kept => {}
        GrabOutcome::Superseded(mut grab) => grab.ended(state),
        GrabOutcome::Ended(mut grab) => {
            grab.ended(state);
            finish(state, seat);
        }
    }
    true
}
