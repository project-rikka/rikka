use rikka_compositor_desktop::{Drawn, WindowId};
use rikka_compositor_geometry::{Buffer, Logical, Rect, Size};
use rikka_compositor_output::CaptureSource;
use rikka_compositor_protocols::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1;
use rikka_compositor_surface::SurfaceId;
use rikka_compositor_types::OutputId;

use crate::context::{CaptureCtx, CaptureRef, capture_ref};
use crate::render::View;
use crate::windows::Windows;

pub(super) fn buffer_size(view: View) -> Option<Size<i32, Buffer>> {
    let size = pixels(view.area, view.scale);
    (size.w > 0 && size.h > 0).then_some(size)
}

fn pixels(area: Rect<i32, Logical>, scale: f64) -> Size<i32, Buffer> {
    Size::new(
        (f64::from(area.size.w) * scale).round() as i32,
        (f64::from(area.size.h) * scale).round() as i32,
    )
}

impl Windows {
    pub(super) fn window_drawn(&self, window: WindowId) -> Vec<Drawn> {
        let mut drawn = Vec::new();
        self.desktop
            .drawn_window_on_wallpapers_into(window, &mut drawn);
        drawn
    }

    fn window_contains(&self, window: WindowId, root: SurfaceId) -> bool {
        self.window_drawn(window).iter().any(
            |drawn| matches!(drawn, Drawn::Surface { root: drawn_root, .. } if *drawn_root == root),
        )
    }
}

impl CaptureRef<'_> {
    pub(super) fn window_of_handle(&self, handle: &ExtForeignToplevelHandleV1) -> Option<WindowId> {
        let foreign = self.shell.published_toplevel_of(handle)?;
        self.windows
            .desktop
            .policies()
            .find(|(_, policy)| policy.foreign == Some(foreign))
            .map(|(id, _)| id)
    }

    pub(super) fn window_output(&self, window: WindowId) -> Option<OutputId> {
        self.windows
            .desktop
            .window(window)?
            .output()
            .or_else(|| self.outputs.enabled().next().map(|output| output.id()))
    }

    pub(super) fn window_view(&self, window: WindowId) -> Option<View> {
        let area = self.windows.desktop.window(window)?.content();
        let scale = self
            .window_output(window)
            .and_then(|output| self.outputs.get(output))
            .map_or(1.0, |output| output.scale());
        Some(View { area, scale })
    }
}

impl CaptureCtx<'_> {
    pub(crate) fn capture_committed(&mut self, root: SurfaceId) {
        let outputs: Vec<OutputId> = self
            .capture
            .waiting
            .iter()
            .filter_map(|request| match request.info.source {
                CaptureSource::Toplevel(window) => Some(window),
                CaptureSource::Output(_) => None,
            })
            .filter(|window| self.windows.window_contains(*window, root))
            .filter_map(|window| capture_ref!(self).window_output(window))
            .collect();
        for output in outputs {
            self.frames.request(output);
        }
    }

    pub(super) fn stop_departed_window_captures(&mut self) {
        let departed: Vec<WindowId> = self
            .captures
            .sessions()
            .filter_map(|(_, info)| match info.source {
                CaptureSource::Toplevel(window) => Some(window),
                CaptureSource::Output(_) => None,
            })
            .filter(|window| self.windows.desktop.window(*window).is_none())
            .collect();
        for window in departed {
            self.captures.stop_source(CaptureSource::Toplevel(window));
        }
    }
}
