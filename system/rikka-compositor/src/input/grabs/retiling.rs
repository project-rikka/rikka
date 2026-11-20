use rikka_compositor_desktop::{WindowId, Workspace};
use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_seat::{Button, GrabFocus, Motion, PointerGrab, PointerHandle};

use super::release;
use super::request::GrabRequest;
use crate::context::WindowsCtx;
use crate::context::{input_ctx, person, windows_ctx};
use crate::state::Compositor;
use rikka_compositor_tiling::Edges;

#[derive(Debug)]
struct TileResizeGrab {
    window: WindowId,
    workspace: Workspace,
    edges: Edges,
    last: Point<f64, Logical>,
}

impl PointerGrab<Compositor> for TileResizeGrab {
    fn motion(&mut self, pointer: &mut PointerHandle<'_, Compositor>, motion: &Motion) {
        pointer.motion(&Motion {
            under: None,
            ..*motion
        });
        let delta = motion.location - self.last;
        self.last = motion.location;
        pointer
            .state()
            .input
            .grabs
            .requests
            .push(GrabRequest::Retiled {
                output: self.workspace.output(),
                window: self.window,
                edges: self.edges,
                delta,
            });
    }

    fn button(&mut self, pointer: &mut PointerHandle<'_, Compositor>, button: &Button) {
        release(pointer, button);
    }

    fn ended(&mut self, state: &mut Compositor) {
        input_ctx!(state).apply_grab_requests();
        state.input.grabs.requests.end();
        windows_ctx!(state).release_all();
        windows_ctx!(state).tile_resize_ended(self.workspace);
    }
}

impl WindowsCtx<'_> {
    pub(crate) fn start_tile_resize(
        &mut self,
        window: WindowId,
        edges: Edges,
        from: Point<f64, Logical>,
    ) {
        let Some(workspace) = self.windows.tiling.workspace_of(window) else {
            return;
        };
        if self.windows.scrolls(window) {
            self.focus_window(Some(window));
        }
        person!(self).set_pointer_grab(
            TileResizeGrab {
                window,
                workspace,
                edges,
                last: from,
            },
            GrabFocus::Clear,
        );
        self.input.grabs.requests.begin();
    }
}
