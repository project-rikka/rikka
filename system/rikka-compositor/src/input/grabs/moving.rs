use rikka_compositor_desktop::WindowId;
use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_seat::{Button, GrabFocus, Motion, PointerGrab, PointerHandle};

use super::release;
use super::request::GrabRequest;
use crate::context::WindowsCtx;
use crate::context::{input_ctx, person, windows_ctx};
use crate::state::Compositor;

#[derive(Debug)]
pub(crate) struct Drag {
    window: WindowId,
    from: Point<f64, Logical>,
    standing: Point<i32, Logical>,
    last: Point<f64, Logical>,
}

impl Drag {
    pub(crate) fn follow(&mut self, state: &mut Compositor, at: Point<f64, Logical>) {
        self.last = at;
        let location = (self.standing.to_f64() + (at - self.from)).to_i32_round();
        state.input.grabs.requests.push(GrabRequest::Moved {
            window: self.window,
            location,
        });
        windows_ctx!(state).snap_dragged(at);
    }

    pub(crate) fn finish(&self, state: &mut Compositor) {
        input_ctx!(state).apply_grab_requests();
        state.input.grabs.requests.end();
        if windows_ctx!(state).snap_dropped() {
            return;
        }
        if windows_ctx!(state).drop_tile(self.window, self.last) {
            return;
        }
        state.windows.session_changed();
    }
}

#[derive(Debug)]
struct MoveGrab(Drag);

impl PointerGrab<Compositor> for MoveGrab {
    fn motion(&mut self, pointer: &mut PointerHandle<'_, Compositor>, motion: &Motion) {
        pointer.motion(&Motion {
            under: None,
            ..*motion
        });
        self.0.follow(pointer.state(), motion.location);
    }

    fn button(&mut self, pointer: &mut PointerHandle<'_, Compositor>, button: &Button) {
        release(pointer, button);
    }

    fn ended(&mut self, state: &mut Compositor) {
        self.0.finish(state);
    }
}

impl WindowsCtx<'_> {
    pub(crate) fn start_move(&mut self, window: WindowId, from: Point<f64, Logical>) {
        let Some(drag) = self.begin_drag(window, from) else {
            return;
        };
        person!(self).set_pointer_grab(MoveGrab(drag), GrabFocus::Clear);
        self.input.grabs.requests.begin();
    }

    pub(crate) fn begin_drag(
        &mut self,
        window: WindowId,
        from: Point<f64, Logical>,
    ) -> Option<Drag> {
        self.animations.finish_motion(window);
        self.windows.confine(window, false);
        let modes = self.windows.enlargements(window);
        let unfolded = (modes.maximized && !modes.fullscreen)
            .then(|| self.restore_under_pointer(window, from))
            .flatten();
        let restored = unfolded.or_else(|| {
            self.windows
                .snapped(window)
                .and_then(|_| self.unsnap_under_pointer(window, from))
        });
        let standing = restored.or_else(|| {
            self.windows
                .desktop
                .window(window)
                .map(|record| record.geometry().loc)
        })?;
        self.snap_drag_started(window, from);
        Some(Drag {
            window,
            from,
            standing,
            last: from,
        })
    }
}
