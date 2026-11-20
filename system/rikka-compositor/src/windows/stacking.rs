use rikka_compositor_desktop::{Visibility, WindowId, WindowKind};
use rikka_compositor_xwm::XWindowId;

use super::family::FAMILY_DEPTH;
use crate::context::{WindowsCtx, windows_ref};

#[derive(Debug, Default)]
pub(crate) struct XStacking(Vec<XWindowId>);

impl WindowsCtx<'_> {
    pub(crate) fn raise_window(&mut self, window: WindowId) {
        self.raise_with_dialogs(window, 0);
        self.windows.lift_floating();
        self.sync_x11_stacking();
    }

    pub(crate) fn raise_family(&mut self, window: WindowId) {
        let root = windows_ref!(self).family_root(window);
        if root != window {
            self.raise_with_dialogs(root, 0);
        }
        self.raise_window(window);
    }

    fn raise_with_dialogs(&mut self, window: WindowId, depth: usize) {
        if let Err(error) = self.windows.desktop.raise(window) {
            tracing::debug!("cannot raise window: {error}");
        }
        if depth >= FAMILY_DEPTH {
            return;
        }
        for dialog in windows_ref!(self).dialogs_of(window) {
            self.raise_with_dialogs(dialog, depth + 1);
        }
    }

    pub(crate) fn sync_x11_stacking(&mut self) {
        let desktop = &self.windows.desktop;
        let x11 = |window: &WindowId| {
            let record = desktop.window(*window)?;
            match record.kind() {
                WindowKind::X11(x11) if !record.unmanaged() => Some((x11, record.visibility())),
                _ => None,
            }
        };
        let (shown, hidden): (Vec<_>, Vec<_>) = desktop
            .stacking()
            .filter_map(|window| x11(&window))
            .partition(|(_, visibility)| *visibility == Visibility::Shown);
        let order: Vec<XWindowId> = hidden
            .into_iter()
            .chain(shown)
            .map(|(x11, _)| x11)
            .collect();
        if order == self.windows.stacking.0 {
            return;
        }
        if self.x11.restack(&order) {
            self.windows.stacking.0 = order;
        }
    }
}
