use rikka_compositor_desktop::SurfaceTrees;
use rikka_compositor_geometry::{Logical, Point, Rect};
use rikka_compositor_shell::ToplevelId;
use rikka_compositor_surface::{SurfaceId, Surfaces};
use rikka_compositor_types::OutputId;

use crate::context::{InputCtx, InputRef, windows_ctx};

#[derive(Debug, Clone, Copy)]
pub(crate) struct Undragged<'a> {
    surfaces: &'a Surfaces,
    dragged: [Option<SurfaceId>; 2],
}

impl SurfaceTrees for Undragged<'_> {
    fn surface_under(
        &self,
        root: SurfaceId,
        point: Point<f64, Logical>,
    ) -> Option<(SurfaceId, Point<f64, Logical>)> {
        if self.dragged.contains(&Some(root)) {
            return None;
        }
        self.surfaces.surface_under(root, point)
    }

    fn bounding_box(&self, root: SurfaceId) -> Option<Rect<i32, Logical>> {
        self.surfaces.bounding_box(root)
    }
}

impl InputRef<'_> {
    pub(crate) fn hit_surfaces(&self) -> Undragged<'_> {
        let dragged = self.input.seats().map(|seat| {
            self.selection
                .selections
                .drag(seat)
                .and_then(|drag| drag.toplevel)
                .and_then(|toplevel| self.windows.desktop.window_for_toplevel(toplevel))
                .and_then(|window| self.windows.desktop.window(window))
                .and_then(|window| window.surface())
        });
        Undragged {
            surfaces: self.surfaces,
            dragged,
        }
    }
}

impl InputCtx<'_> {
    pub(super) fn dragged_toplevel_moved(&mut self, toplevel: ToplevelId, at: Point<f64, Logical>) {
        windows_ctx!(self).follow_drag(toplevel, at);
    }

    pub(crate) fn drag_icon_followed(&mut self) {
        let now = self.selection.icon(self.input.seats()).map(|icon| icon.at);
        let was = std::mem::replace(&mut self.selection.icon_at, now);
        if was == now {
            return;
        }
        let touched: Vec<OutputId> = [was, now]
            .into_iter()
            .flatten()
            .filter_map(|at| self.outputs.containing(at).map(|output| output.id()))
            .collect();
        for output in touched {
            self.frames.request(output);
        }
    }
}
