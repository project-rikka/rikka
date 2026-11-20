use rikka_compositor_desktop::{Tags, Visibility, WindowId, WindowKind, Workspace};
use rikka_compositor_geometry::{Logical, Rect};
use rikka_compositor_scrolling::Sizing;
use rikka_compositor_shell::ToplevelStates;
use rikka_compositor_types::OutputId;
use rikka_compositor_xwm::WindowType;

use crate::policy::WindowRules;

use super::tiling::Kind;
use super::{Enlargements, Windows};
use crate::context::{WindowsCtx, windows_ref};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Placing {
    Travel,
    Stretch,
    Follow,
    Glide(f64),
}

impl Windows {
    pub(crate) fn tiles(&self, window: WindowId) -> bool {
        let Some(record) = self.desktop.window(window) else {
            return false;
        };
        self.tiling_mode()
            && !record.unmanaged()
            && !self
                .desktop
                .policy(window)
                .is_some_and(|policy| policy.floated)
            && !record.tags().contains(Tags::DIALOG)
    }

    pub(crate) fn set_rules(&mut self, rules: WindowRules) -> Option<WindowRules> {
        let previous = std::mem::replace(&mut self.rules, rules);
        (previous != rules).then_some(previous)
    }

    fn tile_window_beside(
        &mut self,
        window: WindowId,
        beside: Option<WindowId>,
    ) -> Option<OutputId> {
        if !self.tiles(window) || self.is_tiled(window) {
            return None;
        }
        let record = self.desktop.window(window)?;
        let workspace = record.workspace()?;
        let standing = record.geometry();
        let next_to = beside.filter(|beside| self.tiling.workspace_of(*beside) == Some(workspace));
        if let Some(policy) = self.desktop.policy_mut(window) {
            policy.mode.tiled_from(standing);
        }
        self.place_tile(workspace, window, next_to);
        Some(workspace.output())
    }

    pub(crate) fn place_tile(
        &mut self,
        workspace: Workspace,
        window: WindowId,
        next_to: Option<WindowId>,
    ) {
        let frame = self.frame(workspace.output());
        let modes = self.enlargements(window);
        self.tiling.insert(workspace, frame, window, next_to);
        self.tiling.enlarge(window, sizing(modes));
    }

    pub(crate) fn confine(&mut self, window: WindowId, confined: bool) {
        if let Some(record) = self.desktop.window_mut(window) {
            record.set_confined(confined);
        }
    }

    fn has_tile_configure(&self, window: WindowId, tile: Rect<i32, Logical>) -> bool {
        self.desktop
            .window(window)
            .is_some_and(|record| match record.kind() {
                WindowKind::Xdg(_) => self.desktop.policy(window).is_some_and(|policy| {
                    policy.configure.size == Some(tile.size)
                        && policy
                            .configure
                            .states
                            .difference(ToplevelStates::ACTIVATED)
                            == policy.mode.states(true)
                }),
                WindowKind::X11(_) => true,
            })
    }

    fn left_to_right(&self, windows: &mut [WindowId]) {
        windows.sort_by_key(|window| {
            self.desktop
                .window(*window)
                .map(|record| record.geometry().loc.x)
        });
    }

    pub(crate) fn lift_floating(&mut self) {
        if !self.tiling_mode() {
            return;
        }
        let order: Vec<WindowId> = self
            .desktop
            .stacking()
            .filter(|window| {
                self.desktop
                    .window(*window)
                    .is_some_and(|record| !matches!(record.visibility(), Visibility::Exiting(_)))
            })
            .collect();
        let standing = |windows: &Windows, window: WindowId| {
            if windows.enlargements(window).fullscreen {
                2
            } else if windows.is_tiled(window) {
                0
            } else {
                1
            }
        };
        let in_order = order
            .windows(2)
            .all(|pair| standing(self, pair[0]) <= standing(self, pair[1]));
        if in_order {
            return;
        }
        let mut lifted: Vec<(u8, WindowId)> = order
            .into_iter()
            .map(|window| (standing(self, window), window))
            .filter(|(standing, _)| *standing > 0)
            .collect();
        lifted.sort_by_key(|(standing, _)| *standing);
        for (_, window) in lifted {
            if let Err(error) = self.desktop.raise(window) {
                tracing::debug!("cannot raise floating window: {error}");
            }
        }
    }
}

pub(crate) fn is_tileable(window_type: WindowType, transient: bool) -> bool {
    !transient && window_type == WindowType::Normal
}

pub(crate) fn sizing(modes: Enlargements) -> Sizing {
    match (modes.fullscreen, modes.maximized) {
        (true, _) => Sizing::Fullscreen,
        (false, true) => Sizing::Maximized,
        (false, false) => Sizing::Normal,
    }
}

impl WindowsCtx<'_> {
    pub(crate) fn tile_window(&mut self, window: WindowId) -> Option<OutputId> {
        let focused = windows_ref!(self)
            .focused_window()
            .filter(|focused| *focused != window);
        self.windows.tile_window_beside(window, focused)
    }

    pub(crate) fn forget_tile(&mut self, window: WindowId) {
        self.windows.confine(window, false);
        if let Some(workspace) = self.windows.tiling.remove(window)
            && self.windows.showing(workspace.output()) == Some(workspace)
        {
            self.retile(workspace.output());
        }
    }

    pub(crate) fn work_area_changed(&mut self, output: OutputId) {
        self.retile(output);
        self.reshape_enlarged(output);
        self.refit_snapped(output);
    }

    pub(crate) fn retile(&mut self, output: OutputId) {
        self.lay_out(output, Placing::Travel);
    }

    pub(crate) fn retile_dragged(&mut self, output: OutputId) {
        self.lay_out(output, Placing::Stretch);
    }

    pub(crate) fn lay_out(&mut self, output: OutputId, placing: Placing) {
        let Some(workspace) = self.windows.showing(output) else {
            return;
        };
        let frame = self.windows.frame(output);
        let laid = self
            .windows
            .tiling
            .on_mut(workspace)
            .map(|layout| (layout.strip().is_some(), layout.arrange(frame)));
        if let Some((scrolls, tiles)) = laid {
            for (window, tile) in tiles {
                self.windows.confine(window, scrolls);
                if !scrolls && self.windows.enlargements(window).any() {
                    continue;
                }
                let standing = self
                    .windows
                    .desktop
                    .window(window)
                    .map(|record| record.geometry());
                if standing == Some(tile) && self.windows.has_tile_configure(window, tile) {
                    continue;
                }
                self.place(window, tile, placing);
            }
        }
        self.windows.lift_floating();
    }

    fn place(&mut self, window: WindowId, tile: Rect<i32, Logical>, placing: Placing) {
        match placing {
            Placing::Travel => {
                self.travel(window, tile);
            }
            Placing::Glide(speed) => {
                self.travel_at(window, tile, Some(speed));
            }
            Placing::Follow => {
                self.animations.finish_motion(window);
                self.stand(window, tile);
            }
            Placing::Stretch => {
                if !self.windows.is_requesting(window, tile) && self.stand(window, tile) {
                    self.stretch_to(window, tile, None);
                }
            }
        }
    }

    pub(crate) fn retile_all(&mut self) {
        let outputs: Vec<OutputId> = self.windows.desktop.outputs().map(|(id, _)| id).collect();
        for output in outputs {
            self.retile(output);
        }
    }

    pub(crate) fn apply_window_rules(&mut self, rules: WindowRules) {
        let Some(previous) = self.windows.set_rules(rules) else {
            return;
        };
        if previous.mode != rules.mode {
            let kind = Kind::from_mode(rules.mode);
            match (previous.mode.tiles(), rules.mode.tiles()) {
                (false, true) => {
                    self.forget_every_snap();
                    self.enter_tiling(kind);
                }
                (true, false) => self.leave_tiling(kind),
                (true, true) => self.rearrange_tiling(kind),
                (false, false) => {}
            }
        } else if previous.gaps != rules.gaps {
            self.retile_all();
        }
        if previous.focus_outline != rules.focus_outline {
            let current = self.windows.recency.current();
            self.redraw_outlines(current, None);
        }
        self.windows.session_changed();
    }

    fn enter_tiling(&mut self, kind: Kind) {
        self.windows.tiling.set_kind(kind);
        let mut windows: Vec<WindowId> = self.windows.desktop.stacking().collect();
        match kind {
            Kind::Dwindle => {
                for window in windows {
                    self.tile_window(window);
                }
            }
            Kind::Columns => {
                self.windows.left_to_right(&mut windows);
                for window in windows {
                    self.windows.tile_window_beside(window, None);
                }
            }
        }
        self.rearrange_every_display();
    }

    fn leave_tiling(&mut self, kind: Kind) {
        for window in self.windows.tiling.rearranged(kind) {
            self.windows.confine(window, false);
            self.restore_floating(window);
        }
        self.rearrange_every_display();
    }

    fn rearrange_tiling(&mut self, kind: Kind) {
        let mut windows = self.windows.tiling.rearranged(kind);
        self.windows.left_to_right(&mut windows);
        for window in windows {
            let workspace = self
                .windows
                .desktop
                .window(window)
                .and_then(|record| record.workspace());
            if let Some(workspace) = workspace {
                self.windows.place_tile(workspace, window, None);
            }
        }
        self.rearrange_every_display();
    }

    fn rearrange_every_display(&mut self) {
        if let Some(focused) = windows_ref!(self).focused_window()
            && let Some(workspace) = self.windows.tiling.workspace_of(focused)
            && let Some(strip) = self.windows.tiling.strip_mut(workspace)
        {
            strip.activate(&focused);
        }
        let outputs: Vec<OutputId> = self.windows.desktop.outputs().map(|(id, _)| id).collect();
        for output in outputs {
            self.retile(output);
            self.reshape_enlarged(output);
        }
    }

    pub(crate) fn toggle_floating(&mut self, window: WindowId) {
        if !self.windows.tiling_mode() {
            return;
        }
        let floated = self
            .windows
            .desktop
            .policy(window)
            .is_some_and(|policy| policy.floated);
        if let Some(policy) = self.windows.desktop.policy_mut(window) {
            policy.floated = !floated;
        }
        if floated {
            if let Some(output) = self.tile_window(window) {
                self.retile(output);
            }
        } else if let Some(workspace) = self.windows.tiling.remove(window) {
            self.windows.confine(window, false);
            self.restore_floating(window);
            self.retile(workspace.output());
        }
        self.raise_window(window);
        self.windows.session_changed();
    }

    pub(crate) fn restore_floating(&mut self, window: WindowId) {
        let Some((record, policy)) = self.windows.desktop.window_and_policy_mut(window) else {
            return;
        };
        if policy.mode.enlarged().is_some() {
            return;
        }
        let back = policy.mode.untiled().unwrap_or(record.geometry());
        self.stand(window, back);
    }

    pub(crate) fn relayout(&mut self, output: OutputId) {
        self.retile(output);
        self.reshape_enlarged(output);
        self.refocus(Some(output));
        self.sync_x11_stacking();
        self.pointer_moved_under();
        self.desktop_changed();
    }
}
