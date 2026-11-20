use rikka_compositor_desktop::{WindowId, Workspace};
use rikka_compositor_scrolling::{Direction, Side};

use crate::context::{WindowsCtx, WindowsRef, windows_ref};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColumnCommand {
    Focus(Direction),
    Shift(Direction),
    ConsumeOrExpel(Side),
    CycleWidth,
}

impl WindowsCtx<'_> {
    pub(crate) fn scroll_to(&mut self, window: WindowId) {
        let Some(workspace) = self.windows.tiling.workspace_of(window) else {
            return;
        };
        let moved = self
            .windows
            .tiling
            .strip_mut(workspace)
            .is_some_and(|strip| strip.activate(&window));
        if moved && self.windows.showing(workspace.output()) == Some(workspace) {
            self.retile(workspace.output());
        }
    }

    pub(crate) fn retile_around(&mut self, window: WindowId) {
        if let Some(workspace) = self.windows.tiling.workspace_of(window)
            && self.windows.showing(workspace.output()) == Some(workspace)
        {
            self.retile(workspace.output());
        }
    }

    pub(crate) fn command_columns(&mut self, command: ColumnCommand) {
        let Some((workspace, focused)) = windows_ref!(self).commanded_strip() else {
            return;
        };
        let Some(strip) = self.windows.tiling.strip_mut(workspace) else {
            return;
        };
        if let Some(focused) = focused {
            strip.activate(&focused);
        }
        let changed = match command {
            ColumnCommand::Focus(direction) => strip.focus(direction).is_some(),
            ColumnCommand::Shift(direction) => strip.shift(direction),
            ColumnCommand::ConsumeOrExpel(side) => strip.consume_or_expel(side),
            ColumnCommand::CycleWidth => strip.cycle_width(),
        };
        if !changed {
            return;
        }
        let active = strip.active();
        self.retile(workspace.output());
        let moved_focus = matches!(command, ColumnCommand::Focus(_)) || focused.is_some();
        if moved_focus && active != focused {
            self.focus_window(active);
        }
        self.windows.session_changed();
    }
}

impl WindowsRef<'_> {
    fn commanded_strip(&self) -> Option<(Workspace, Option<WindowId>)> {
        let scrolled = |workspace: Workspace| self.windows.tiling.strip(workspace).is_some();
        if let Some(focused) = self.focused_window()
            && let Some(workspace) = self.windows.tiling.workspace_of(focused)
            && scrolled(workspace)
        {
            return Some((workspace, Some(focused)));
        }
        let workspace = self.windows.showing(self.active_output()?)?;
        scrolled(workspace).then_some((workspace, None))
    }
}
