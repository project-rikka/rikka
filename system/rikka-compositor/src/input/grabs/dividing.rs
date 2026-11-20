use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_seat::{Button, GrabFocus, Motion, PointerGrab, PointerHandle};

use super::release;
use super::request::GrabRequest;
use crate::context::WindowsCtx;
use crate::context::{input_ctx, person, windows_ctx};
use crate::snap::{Divider, Group};
use crate::state::Compositor;

#[derive(Debug)]
pub(crate) struct Divide {
    group: Group,
    divider: Divider,
    from: Point<f64, Logical>,
    delta: i32,
}

impl Divide {
    pub(crate) fn follow(&mut self, state: &mut Compositor, at: Point<f64, Logical>) {
        let delta = self.divider.takes(at - self.from);
        if delta == self.delta {
            return;
        }
        self.delta = delta;
        let moved = self.group.moved(self.divider, delta);
        state.input.grabs.requests.push(GrabRequest::Divided(moved));
    }

    pub(crate) fn finish(&self, state: &mut Compositor) {
        input_ctx!(state).apply_grab_requests();
        state.input.grabs.requests.end();
        for window in &self.group.windows {
            windows_ctx!(state).release_window(*window);
        }
    }
}

#[derive(Debug)]
struct DividerGrab(Divide);

impl PointerGrab<Compositor> for DividerGrab {
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
    pub(crate) fn start_divider(
        &mut self,
        group: Group,
        divider: Divider,
        from: Point<f64, Logical>,
    ) {
        let divide = self.begin_divide(group, divider, from);
        person!(self).set_pointer_grab(DividerGrab(divide), GrabFocus::Clear);
        self.input.grabs.requests.begin();
    }

    pub(crate) fn begin_divide(
        &mut self,
        group: Group,
        divider: Divider,
        from: Point<f64, Logical>,
    ) -> Divide {
        for window in &group.windows {
            self.animations.finish_motion(*window);
        }
        Divide {
            group,
            divider,
            from,
            delta: 0,
        }
    }
}
