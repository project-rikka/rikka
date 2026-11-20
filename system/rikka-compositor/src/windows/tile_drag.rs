use rikka_compositor_desktop::{WindowId, Workspace};
use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_types::OutputId;

use super::Windows;
use crate::context::WindowsCtx;
use rikka_compositor_tiling::{EDGE_REACH, Edges};

impl Windows {
    pub(crate) fn tile_edge_at(&self, at: Point<f64, Logical>) -> Option<(WindowId, Edges)> {
        if !self.rules().resize_on_border {
            return None;
        }
        let output = self.desktop.output_at(at)?;
        self.tiling
            .layout(self.showing(output)?)?
            .edge_at(self.frame(output), at, EDGE_REACH)
    }
}

impl WindowsCtx<'_> {
    pub(crate) fn resize_tile(
        &mut self,
        output: OutputId,
        window: WindowId,
        edges: Edges,
        delta: Point<f64, Logical>,
    ) {
        let Some(showing) = self.windows.showing(output) else {
            return;
        };
        let frame = self.windows.frame(output);
        let moved = self
            .windows
            .tiling
            .on_mut(showing)
            .is_some_and(|layout| layout.resize(frame, window, edges, delta));
        if moved {
            self.retile_dragged(output);
        }
    }

    pub(crate) fn tile_resize_ended(&mut self, workspace: Workspace) {
        let Some(strip) = self.windows.tiling.strip_mut(workspace) else {
            return;
        };
        strip.release();
        if self.windows.showing(workspace.output()) == Some(workspace) {
            self.retile(workspace.output());
        }
    }

    pub(crate) fn drop_tile(&mut self, window: WindowId, at: Point<f64, Logical>) -> bool {
        let Some(from) = self.windows.tiling.workspace_of(window) else {
            return false;
        };
        let output = self
            .windows
            .desktop
            .output_at(at)
            .unwrap_or_else(|| from.output());
        let Some(onto) = self.windows.showing(output) else {
            return false;
        };
        let frame = self.windows.frame(output);
        if onto == from {
            if let Some(layout) = self.windows.tiling.on_mut(onto) {
                layout.rearrange(frame, window, at);
            }
        } else {
            let target = self
                .windows
                .tiling
                .layout(onto)
                .and_then(|layout| layout.window_at(frame, at));
            self.windows.tiling.remove(window);
            if let Err(error) = self.windows.desktop.move_to_workspace(window, onto) {
                tracing::debug!("cannot move tile to output: {error}");
            }
            self.windows.place_tile(onto, window, target);
            self.retile(from.output());
        }
        self.retile(output);
        if self.windows.scrolls(window) {
            self.focus_window(Some(window));
        }
        self.windows.session_changed();
        true
    }
}
