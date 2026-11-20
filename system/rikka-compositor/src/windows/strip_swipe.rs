use std::time::Duration;

use rikka_compositor_types::OutputId;

use super::arrange::Placing;
use crate::context::{WindowsCtx, windows_ref};
use crate::windows::Windows;

impl Windows {
    pub(crate) fn begin_strip_swipe(&mut self, output: OutputId) {
        let Some(workspace) = self.showing(output) else {
            return;
        };
        let began = self
            .tiling
            .strip_mut(workspace)
            .is_some_and(|strip| strip.swipe_begin());
        self.swiped = began.then_some(workspace);
    }
}

impl WindowsCtx<'_> {
    pub(crate) fn strip_swiped(&mut self, dx: f64, time: Duration) {
        let Some(workspace) = self.windows.swiped else {
            return;
        };
        let Some(strip) = self.windows.tiling.strip_mut(workspace) else {
            self.windows.swiped = None;
            return;
        };
        strip.swipe_update(dx, time);
        if self.windows.showing(workspace.output()) == Some(workspace) {
            self.lay_out(workspace.output(), Placing::Follow);
        }
    }

    pub(crate) fn end_strip_swipe(&mut self, time: Duration) {
        let Some(workspace) = self.windows.swiped.take() else {
            return;
        };
        let Some(strip) = self.windows.tiling.strip_mut(workspace) else {
            return;
        };
        let speed = strip.swipe_end(time);
        let active = strip.active();
        if self.windows.showing(workspace.output()) == Some(workspace) {
            self.lay_out(workspace.output(), Placing::Glide(-speed));
        }
        if active.is_some() && active != windows_ref!(self).focused_window() {
            self.focus_window(active);
        }
        self.windows.session_changed();
    }
}
