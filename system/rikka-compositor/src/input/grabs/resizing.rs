use rikka_compositor_desktop::WindowId;
use rikka_compositor_geometry::{Logical, Point, Rect, Size};
use rikka_compositor_seat::{Button, GrabFocus, Motion, PointerGrab, PointerHandle};

use super::dividing::Divide;
use super::release;
use super::request::GrabRequest;
use crate::state::Compositor;
use rikka_compositor_tiling::Edges;

use crate::context::WindowsCtx;
use crate::context::{input_ctx, person, windows_ctx};
use crate::windows::Anchor;

#[derive(Debug)]
pub(crate) struct Resize {
    window: WindowId,
    edges: Edges,
    from: Point<f64, Logical>,
    start: Rect<i32, Logical>,
    size: Size<i32, Logical>,
    limits: (Size<i32, Logical>, Size<i32, Logical>),
}

impl Resize {
    fn anchor(&self) -> Anchor {
        Anchor::new(self.edges, self.start)
    }

    fn requested_rect(&self) -> Rect<i32, Logical> {
        Rect::new(self.anchor().place(self.size, self.start.loc), self.size)
    }

    fn request(&self) -> GrabRequest {
        GrabRequest::Resized {
            window: self.window,
            rect: self.requested_rect(),
            anchor: self.anchor(),
        }
    }

    fn follow(&mut self, state: &mut Compositor, at: Point<f64, Logical>) {
        self.size = stretched(self.start.size, self.edges, at - self.from, self.limits);
        state.input.grabs.requests.push(self.request());
    }

    fn finish(&self, state: &mut Compositor) {
        state.windows.resizing(self.window, false);
        state.input.grabs.requests.push(self.request());
        input_ctx!(state).apply_grab_requests();
        windows_ctx!(state).release_window(self.window);
        state.input.grabs.requests.end();
    }
}

pub(crate) fn stretched(
    start: Size<i32, Logical>,
    edges: Edges,
    delta: Point<f64, Logical>,
    (least, most): (Size<i32, Logical>, Size<i32, Logical>),
) -> Size<i32, Logical> {
    let mut size = start.to_f64();
    if edges.contains(Edges::LEFT) {
        size.w -= delta.x;
    } else if edges.contains(Edges::RIGHT) {
        size.w += delta.x;
    }
    if edges.contains(Edges::TOP) {
        size.h -= delta.y;
    } else if edges.contains(Edges::BOTTOM) {
        size.h += delta.y;
    }
    let bound = |value: f64, least: i32, most: i32| {
        let least = f64::from(least.max(1));
        let most = if most > 0 {
            f64::from(most).max(least)
        } else {
            f64::MAX
        };
        value.clamp(least, most) as i32
    };
    Size::new(
        bound(size.w, least.w, most.w),
        bound(size.h, least.h, most.h),
    )
}

#[derive(Debug)]
pub(crate) enum Reshape {
    Window(Resize),
    Divider(Divide),
}

impl Reshape {
    pub(crate) fn follow(&mut self, state: &mut Compositor, at: Point<f64, Logical>) {
        match self {
            Self::Window(resize) => resize.follow(state, at),
            Self::Divider(divide) => divide.follow(state, at),
        }
    }

    pub(crate) fn finish(&self, state: &mut Compositor) {
        match self {
            Self::Window(resize) => resize.finish(state),
            Self::Divider(divide) => divide.finish(state),
        }
    }
}

#[derive(Debug)]
struct ReshapeGrab(Reshape);

impl PointerGrab<Compositor> for ReshapeGrab {
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
    pub(crate) fn start_resize(
        &mut self,
        window: WindowId,
        edges: Edges,
        from: Point<f64, Logical>,
    ) {
        if self.windows.is_tiled(window) && !edges.is_empty() {
            self.animations.finish_motion(window);
            return self.start_tile_resize(window, edges, from);
        }
        let Some(reshape) = self.begin_reshape(window, edges, from) else {
            return;
        };
        person!(self).set_pointer_grab(ReshapeGrab(reshape), GrabFocus::Clear);
        self.input.grabs.requests.begin();
    }

    pub(crate) fn begin_reshape(
        &mut self,
        window: WindowId,
        edges: Edges,
        from: Point<f64, Logical>,
    ) -> Option<Reshape> {
        if edges.is_empty() || self.windows.is_tiled(window) {
            return None;
        }
        self.animations.finish_motion(window);
        if let Some((group, divider)) = self.snap_divider_pulled(window, edges) {
            return Some(Reshape::Divider(self.begin_divide(group, divider, from)));
        }
        let start = self.windows.desktop.window(window)?.geometry();
        let resize = Resize {
            window,
            edges,
            from,
            start,
            size: start.size,
            limits: self.size_limits(window),
        };
        self.windows.resizing(window, true);
        self.stretch_to(window, start, Some(resize.anchor()));
        Some(Reshape::Window(resize))
    }
}
