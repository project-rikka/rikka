use rikka_compositor_desktop::{SnapshotId, WindowId};
use rikka_compositor_geometry::{Logical, Rect};

use crate::context::WindowsCtx;
use crate::glass::frosted;

impl WindowsCtx<'_> {
    pub(crate) fn exit_animated(&mut self, window: WindowId) -> bool {
        let Some((snapshot, geometry)) = self.freeze_window(window) else {
            return false;
        };
        match self.windows.desktop.exit_window(window, snapshot) {
            Ok(true) => {
                self.animations.close(window, snapshot, geometry);
                self.frames.request_all();
                true
            }
            Ok(false) => {
                self.scenes.thaw(snapshot);
                true
            }
            Err(error) => {
                tracing::debug!("cannot snapshot closing window: {error}");
                self.scenes.thaw(snapshot);
                false
            }
        }
    }

    pub(crate) fn animate_exit(&mut self, window: WindowId) {
        if let Some((snapshot, geometry)) = self.freeze_window(window) {
            self.animations.put_away(snapshot, geometry);
            self.frames.request_all();
        }
    }

    pub(crate) fn freeze_leaving(
        &mut self,
        windows: &[WindowId],
    ) -> Vec<(SnapshotId, Rect<i32, Logical>)> {
        windows
            .iter()
            .filter_map(|window| self.freeze_window(*window))
            .collect()
    }

    fn freeze_window(&mut self, window: WindowId) -> Option<(SnapshotId, Rect<i32, Logical>)> {
        if !self.animations.enabled() {
            return None;
        }
        let record = self.windows.desktop.window(window)?;
        let (root, geometry) = (record.surface()?, record.geometry());
        let frosted = frosted(
            self.surfaces,
            root,
            record.surface_location() - geometry.loc,
        );
        let snapshot = self.animations.mint();
        self.scenes
            .freeze(root, snapshot, geometry, &frosted)
            .then_some((snapshot, geometry))
    }
}
