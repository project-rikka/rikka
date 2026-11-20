use rikka_compositor_desktop::{WindowId, WindowKind};
use rikka_compositor_geometry::{Logical, Point, Rect};
use rikka_compositor_shell::ToplevelStates;

use crate::context::WindowsCtx;

impl WindowsCtx<'_> {
    pub(crate) fn stand(&mut self, window: WindowId, rect: Rect<i32, Logical>) -> bool {
        self.frames.request_all();
        let tiled = self.windows.is_tiled(window);
        let Some((record, policy)) = self.windows.desktop.window_and_policy_mut(window) else {
            return false;
        };
        record.set_placement(policy.mode.placement(tiled));
        self.refresh_frame(window);
        let Some((record, policy)) = self.windows.desktop.window_and_policy_mut(window) else {
            return false;
        };
        match record.kind() {
            WindowKind::Xdg(_) => {
                let mut geometry = record.geometry();
                geometry.loc = rect.loc;
                record.set_geometry(geometry);
                let configure = &mut policy.configure;
                configure.size = Some(rect.size);
                configure.states =
                    (configure.states & ToplevelStates::ACTIVATED) | policy.mode.states(tiled);
                self.windows.queue_configure(window);
                true
            }
            WindowKind::X11(_) => self.configure_x11(window, rect),
        }
    }

    pub(crate) fn move_window(&mut self, window: WindowId, to: Point<i32, Logical>) {
        self.frames.request_all();
        let Some(record) = self.windows.desktop.window(window) else {
            return;
        };
        let rect = Rect::new(to, record.geometry().size);
        match record.kind() {
            WindowKind::Xdg(_) => {
                if let Some(record) = self.windows.desktop.window_mut(window) {
                    record.set_geometry(rect);
                }
            }
            WindowKind::X11(_) => {
                self.configure_x11(window, rect);
            }
        }
    }

    pub(crate) fn close_window(&mut self, window: WindowId) {
        let Some(kind) = self
            .windows
            .desktop
            .window(window)
            .map(|window| window.kind())
        else {
            return;
        };
        match kind {
            WindowKind::Xdg(toplevel) => self.shell.close_toplevel(toplevel),
            WindowKind::X11(x11) => self.x11.close(x11),
        }
    }
}
