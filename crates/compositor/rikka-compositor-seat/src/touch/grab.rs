use std::fmt;

use crate::access::Access;
use crate::event::SeatEvent;
use crate::grab_slot::GrabOutcome;
use crate::host::SeatHost;
use crate::ids::SeatId;
use crate::pointer::GrabFocus;
use crate::touch::deliver::{
    touch_cancel, touch_down, touch_frame, touch_motion, touch_shape, touch_up,
};
use crate::touch::events::{TouchDown, TouchMotion, TouchShape, TouchUp};

pub trait TouchGrab<S: SeatHost>: fmt::Debug + 'static {
    fn down(&mut self, touch: &mut TouchHandle<'_, S>, down: &TouchDown) {
        touch.down(down);
    }

    fn motion(&mut self, touch: &mut TouchHandle<'_, S>, motion: &TouchMotion) {
        touch.motion(motion);
    }

    fn up(&mut self, touch: &mut TouchHandle<'_, S>, up: &TouchUp) {
        touch.up(up);
    }

    fn shape(&mut self, touch: &mut TouchHandle<'_, S>, shape: &TouchShape) {
        touch.shape(shape);
    }

    fn frame(&mut self, touch: &mut TouchHandle<'_, S>) {
        touch.frame();
    }

    fn cancel(&mut self, touch: &mut TouchHandle<'_, S>) {
        touch.cancel();
    }

    fn ended(&mut self, _state: &mut S) {}
}

pub struct TouchHandle<'a, S: SeatHost> {
    state: &'a mut S,
    seat: SeatId,
}

impl<S: SeatHost> TouchHandle<'_, S> {
    pub fn state(&mut self) -> &mut S {
        self.state
    }

    pub fn seat(&self) -> SeatId {
        self.seat
    }

    pub fn active(&mut self) -> bool {
        self.state
            .seats()
            .get(self.seat)
            .is_some_and(|entry| entry.touch.active())
    }

    pub fn down(&mut self, down: &TouchDown) {
        touch_down(self.state, self.seat, down);
    }

    pub fn motion(&mut self, motion: &TouchMotion) {
        touch_motion(self.state, self.seat, motion);
    }

    pub fn up(&mut self, up: &TouchUp) {
        touch_up(self.state, self.seat, up);
    }

    pub fn shape(&mut self, shape: &TouchShape) {
        touch_shape(self.state, self.seat, shape);
    }

    pub fn frame(&mut self) {
        touch_frame(self.state, self.seat);
    }

    pub fn cancel(&mut self) {
        touch_cancel(self.state, self.seat);
    }

    pub fn end_grab(&mut self) {
        end_grab::<S>(self.state, self.seat);
    }
}

impl<S: SeatHost> fmt::Debug for TouchHandle<'_, S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TouchHandle")
            .field("seat", &self.seat)
            .finish_non_exhaustive()
    }
}

pub(crate) fn set_grab<A: Access>(
    state: &mut A,
    seat: SeatId,
    grab: impl TouchGrab<A::Host>,
    focus: GrabFocus,
) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let erased: Box<dyn TouchGrab<A::Host>> = Box::new(grab);
    if let Some(previous) = entry.touch.grab.install(erased) {
        state.touch_grab_ended(previous);
    }
    if focus == GrabFocus::Clear {
        touch_cancel(state, seat);
    }
    state.report(SeatEvent::TouchGrab {
        seat,
        grabbed: true,
    });
}

pub(crate) fn end_grab<A: Access>(state: &mut A, seat: SeatId) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    if let Some(Some(grab)) = entry.touch.grab.release::<dyn TouchGrab<A::Host>>() {
        state.touch_grab_ended(grab);
        finish(state, seat);
    }
}

fn finish<A: Access>(state: &mut A, seat: SeatId) {
    state.report(SeatEvent::TouchGrab {
        seat,
        grabbed: false,
    });
}

pub(crate) fn dispatch<S: SeatHost>(
    state: &mut S,
    seat: SeatId,
    call: impl FnOnce(&mut dyn TouchGrab<S>, &mut TouchHandle<'_, S>),
) -> bool {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return true;
    };
    let Some(mut taken) = entry.touch.grab.take::<dyn TouchGrab<S>>() else {
        return false;
    };
    let mut handle = TouchHandle {
        state: &mut *state,
        seat,
    };
    call(taken.grab(), &mut handle);
    let Some(entry) = state.seats().seat_mut(seat) else {
        return true;
    };
    match entry.touch.grab.restore(taken) {
        GrabOutcome::Kept => {}
        GrabOutcome::Superseded(mut grab) => grab.ended(state),
        GrabOutcome::Ended(mut grab) => {
            grab.ended(state);
            finish(state, seat);
        }
    }
    true
}
