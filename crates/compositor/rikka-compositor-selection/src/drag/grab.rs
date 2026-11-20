use rikka_compositor_seat::{
    Button, Motion, PointerGrab, PointerHandle, SeatId, TouchDown, TouchGrab, TouchHandle,
    TouchMotion, TouchShape, TouchUp,
};

use crate::drag::end::finish;
use crate::drag::focus::moved;
use crate::drag::session::drag_mut;
use crate::host::SelectionHost;

#[derive(Debug)]
pub(crate) struct DragGrab {
    pub(crate) seat: SeatId,
}

impl<S: SelectionHost> PointerGrab<S> for DragGrab {
    fn motion(&mut self, pointer: &mut PointerHandle<'_, S>, motion: &Motion) {
        moved(pointer.state(), self.seat, motion);
    }

    fn button(&mut self, pointer: &mut PointerHandle<'_, S>, _button: &Button) {
        if pointer.pressed() {
            return;
        }
        if let Some(drag) = drag_mut(pointer.state(), self.seat) {
            drag.released = true;
        }
        pointer.end_grab();
    }

    fn ended(&mut self, state: &mut S) {
        finish(state, self.seat);
    }
}

#[derive(Debug)]
pub(crate) struct TouchDragGrab {
    pub(crate) seat: SeatId,
    pub(crate) slot: u32,
}

impl<S: SelectionHost> TouchGrab<S> for TouchDragGrab {
    fn down(&mut self, _touch: &mut TouchHandle<'_, S>, _down: &TouchDown) {}

    fn motion(&mut self, touch: &mut TouchHandle<'_, S>, motion: &TouchMotion) {
        if motion.slot != self.slot {
            return;
        }
        let motion = Motion {
            location: motion.location,
            under: motion.under,
            time: motion.time,
        };
        moved(touch.state(), self.seat, &motion);
    }

    fn up(&mut self, touch: &mut TouchHandle<'_, S>, up: &TouchUp) {
        if up.slot != self.slot {
            return;
        }
        if let Some(drag) = drag_mut(touch.state(), self.seat) {
            drag.released = true;
        }
        touch.end_grab();
    }

    fn shape(&mut self, _touch: &mut TouchHandle<'_, S>, _shape: &TouchShape) {}

    fn frame(&mut self, _touch: &mut TouchHandle<'_, S>) {}

    fn cancel(&mut self, touch: &mut TouchHandle<'_, S>) {
        touch.end_grab();
    }

    fn ended(&mut self, state: &mut S) {
        finish(state, self.seat);
    }
}
