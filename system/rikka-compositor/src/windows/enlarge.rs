use rikka_compositor_desktop::{WindowId, WindowKind};
use rikka_compositor_geometry::{Logical, Rect};
use rikka_compositor_types::OutputId;

use super::arrange::sizing;
use super::mode::{Enlarged, Mode};
use super::{Enlargement, Enlargements, Windows};
use crate::context::WindowsCtx;

impl Windows {
    pub(crate) fn shape_for(
        &self,
        output: OutputId,
        modes: Enlargements,
    ) -> Option<Rect<i32, Logical>> {
        if modes.fullscreen {
            return Some(self.full_area(output));
        }
        modes.maximized.then(|| self.usable_area(output))
    }

    pub(crate) fn mode_of(&self, window: WindowId) -> Mode {
        self.desktop
            .policy(window)
            .map(|policy| policy.mode)
            .unwrap_or_default()
    }

    pub(crate) fn set_mode(&mut self, window: WindowId, mode: Mode) {
        if let Some(policy) = self.desktop.policy_mut(window) {
            policy.mode = mode;
        }
    }
}

impl WindowsCtx<'_> {
    pub(crate) fn toggle(&mut self, window: WindowId, mode: Enlargement) {
        let modes = self.windows.enlargements(window);
        let enabled = match mode {
            Enlargement::Maximized => modes.maximized,
            Enlargement::Fullscreen => modes.fullscreen,
        };
        match enabled {
            true => self.shrink(window, mode),
            false => self.enlarge(window, mode, None),
        }
    }

    pub(crate) fn enlarge(&mut self, window: WindowId, mode: Enlargement, onto: Option<OutputId>) {
        let Some(record) = self.windows.desktop.window(window) else {
            return;
        };
        if record.unmanaged() {
            return;
        }
        let from = record.geometry();
        let current = self.windows.mode_of(window);
        let Some(output) = onto.or_else(|| self.windows.output_of(window)) else {
            return;
        };
        let modes = current.enlargements().with_mode(mode);
        if self.windows.scrolls(window) {
            self.windows
                .set_mode(window, current.enlarged_to(modes, from, true));
            self.sync_x11_maximized(window, modes);
            self.windows.tiling.enlarge(window, sizing(modes));
            self.raise_window(window);
            self.retile_around(window);
            self.windows.session_changed();
            return;
        }
        let Some(to) = self.windows.shape_for(output, modes) else {
            return;
        };
        let tiled = self.windows.is_tiled(window);
        self.windows
            .set_mode(window, current.enlarged_to(modes, from, tiled));
        self.sync_x11_maximized(window, modes);
        self.travel(window, to);
        self.raise_window(window);
        self.windows.session_changed();
    }

    pub(crate) fn toggle_maximized(&mut self, window: WindowId) {
        let resizable = self
            .windows
            .desktop
            .policy(window)
            .and_then(|policy| policy.decorated.as_ref())
            .is_some_and(|decorated| decorated.resizable);
        if !resizable {
            return;
        }
        match self.windows.enlargements(window).maximized {
            true => self.shrink(window, Enlargement::Maximized),
            false => self.enlarge(window, Enlargement::Maximized, None),
        }
    }

    pub(crate) fn shrink(&mut self, window: WindowId, mode: Enlargement) {
        let Some(enlarged) = self.windows.mode_of(window).enlarged() else {
            return;
        };
        let modes = enlarged.modes.without_mode(mode);
        self.sync_x11_maximized(window, modes);
        if self.windows.scrolls(window) {
            let mode = match modes.any() {
                true => Mode::Enlarged(Enlarged { modes, ..enlarged }),
                false => enlarged.shrunk(true),
            };
            self.windows.set_mode(window, mode);
            self.windows.tiling.enlarge(window, sizing(modes));
            self.retile_around(window);
            self.windows.session_changed();
            return;
        }
        let output = self.windows.output_of(window);
        if let Some(to) = output.and_then(|output| self.windows.shape_for(output, modes)) {
            self.windows
                .set_mode(window, Mode::Enlarged(Enlarged { modes, ..enlarged }));
            self.travel(window, to);
            self.windows.session_changed();
            return;
        }
        let tiled = self.windows.tiling.workspace_of(window);
        self.windows
            .set_mode(window, enlarged.shrunk(tiled.is_some()));
        match tiled {
            Some(workspace) => self.retile(workspace.output()),
            None => {
                self.travel(window, enlarged.restore);
            }
        }
        self.windows.session_changed();
    }

    pub(crate) fn reshape_enlarged(&mut self, output: OutputId) {
        let enlarged: Vec<(WindowId, Enlargements)> = self
            .windows
            .desktop
            .policies()
            .filter_map(|(id, policy)| Some((id, policy.mode.enlarged()?.modes)))
            .filter(|(id, _)| self.windows.output_of(*id) == Some(output))
            .filter(|(id, _)| !self.windows.scrolls(*id))
            .collect();
        for (window, modes) in enlarged {
            if let Some(to) = self.windows.shape_for(output, modes) {
                self.stand(window, to);
            }
        }
    }

    pub(crate) fn sync_x11_maximized(&mut self, window: WindowId, modes: Enlargements) {
        let Some(WindowKind::X11(x11)) = self
            .windows
            .desktop
            .window(window)
            .map(|record| record.kind())
        else {
            return;
        };
        self.x11.enlarge(x11, modes);
    }
}
