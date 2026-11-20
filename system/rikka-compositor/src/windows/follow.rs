use rikka_compositor_desktop::{WindowId, WindowKind};
use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_shell::ToplevelId;

use crate::context::InputCtx;
use crate::context::{WindowsCtx, WindowsRef, windows_ctx};
use crate::input::GrabRequest;

impl WindowsCtx<'_> {
    pub(crate) fn follow_drag(&mut self, toplevel: ToplevelId, at: Point<f64, Logical>) {
        let Some(window) = self.windows.desktop.window_for_toplevel(toplevel) else {
            self.follow_arrival(toplevel, at);
            return;
        };
        self.input.grabs.requests.push(GrabRequest::Moved {
            window,
            location: at.to_i32_round(),
        });
        self.frames.input_arrived();
    }

    pub(crate) fn drag_attached(&mut self, toplevel: ToplevelId, offset: Point<i32, Logical>) {
        let Some(window) = self.windows.desktop.window_for_toplevel(toplevel) else {
            let planned = self.windows.arrivals.contains_key(&toplevel)
                || self
                    .shell
                    .toplevel(toplevel)
                    .is_some_and(|state| state.initial_configure_sent());
            if planned {
                self.plan_arrival(toplevel);
            }
            return;
        };
        if self.windows.is_tiled(window) {
            self.forget_tile(window);
        }
        let at = self.cursor.position() - offset.to_f64();
        self.follow_drag(toplevel, at);
    }
}

impl WindowsRef<'_> {
    pub(crate) fn is_dragged(&self, window: WindowId) -> bool {
        match self
            .windows
            .desktop
            .window(window)
            .map(|record| record.kind())
        {
            Some(WindowKind::Xdg(toplevel)) => {
                self.shell.dragged_toplevel_offset(toplevel).is_some()
            }
            _ => false,
        }
    }

    pub(crate) fn dragged_along(&self, toplevel: ToplevelId) -> Option<Point<f64, Logical>> {
        let offset = self.shell.dragged_toplevel_offset(toplevel)?;
        Some(self.cursor.position() - offset.to_f64())
    }
}

impl InputCtx<'_> {
    pub(crate) fn drag_released(&mut self, toplevel: ToplevelId) {
        self.apply_grab_requests();
        match self.windows.desktop.window_for_toplevel(toplevel) {
            Some(window) => {
                if let Some(output) = windows_ctx!(self).tile_window(window) {
                    windows_ctx!(self).retile(output);
                }
                windows_ctx!(self).focus_window(Some(window));
            }
            None => windows_ctx!(self).land_arrival(toplevel),
        }
        self.windows.session_changed();
    }
}
