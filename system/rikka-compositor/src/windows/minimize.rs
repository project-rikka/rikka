use rikka_compositor_desktop::{Visibility, WindowId, WindowKind};

use crate::context::{WindowsCtx, windows_ref};

impl WindowsCtx<'_> {
    fn minimize_one(&mut self, window: WindowId) -> bool {
        if self.windows.is_tiled(window) {
            return false;
        }
        match self.windows.desktop.minimize(window) {
            Ok(true) => self.animate_exit(window),
            Ok(false) => return false,
            Err(error) => {
                tracing::debug!("cannot minimize window: {error}");
                return false;
            }
        }
        self.sync_x11_minimized(window, true);
        true
    }

    pub(crate) fn unminimize_window(&mut self, window: WindowId) {
        let mut restored = false;
        for member in windows_ref!(self).family_of(window) {
            restored |= self.unminimize_one(member);
        }
        if restored {
            self.desktop_changed();
        }
    }

    fn unminimize_one(&mut self, window: WindowId) -> bool {
        match self.windows.desktop.unminimize(window) {
            Ok(true) => self.animations.open(window),
            Ok(false) => return false,
            Err(error) => {
                tracing::debug!("cannot restore window: {error}");
                return false;
            }
        }
        self.sync_x11_minimized(window, false);
        true
    }

    pub(crate) fn restore_every_minimized_window(&mut self) {
        let away: Vec<WindowId> = self
            .windows
            .desktop
            .windows()
            .filter(|(_, record)| record.visibility() == Visibility::Minimized)
            .map(|(id, _)| id)
            .collect();
        if away.is_empty() {
            return;
        }
        tracing::debug!(
            "restored {} minimized window(s): taskbar is hidden",
            away.len()
        );
        for window in away {
            self.unminimize_window(window);
        }
    }

    fn sync_x11_minimized(&mut self, window: WindowId, minimized: bool) {
        let Some(WindowKind::X11(x11)) = self
            .windows
            .desktop
            .window(window)
            .map(|record| record.kind())
        else {
            return;
        };
        self.x11.minimize(x11, minimized);
    }

    pub(crate) fn minimize_window(&mut self, window: WindowId) {
        if !self.policy.minimizes_windows() {
            return tracing::debug!("minimize ignored: taskbar is hidden");
        }
        if self
            .windows
            .is_tiled(windows_ref!(self).family_root(window))
        {
            return;
        }
        let output = self.windows.output_of(window);
        let mut minimized = false;
        for member in windows_ref!(self).family_of(window) {
            minimized |= self.minimize_one(member);
        }
        if minimized {
            self.refocus(output);
            self.desktop_changed();
        }
    }

    pub(crate) fn reveal_window(&mut self, window: WindowId) {
        self.unminimize_window(window);
        let hidden = self.windows.desktop.window(window).and_then(|record| {
            (record.visibility() == Visibility::OnHiddenWorkspace)
                .then(|| record.workspace())
                .flatten()
        });
        if let Some(workspace) = hidden {
            self.show_workspace_on(workspace.output(), workspace.number());
        }
        self.focus_window(Some(window));
    }
}
