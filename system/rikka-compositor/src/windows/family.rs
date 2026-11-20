use rikka_compositor_desktop::{WindowId, WindowKind};
use rikka_compositor_geometry::{Logical, Point, Size};
use rikka_compositor_shell::ToplevelId;
use rikka_compositor_xwm::WindowStates;

use super::Windows;
use crate::context::WindowsRef;

pub(super) const FAMILY_DEPTH: usize = 16;

fn centered(over: i32, over_length: i32, length: i32, area: i32, area_length: i32) -> i32 {
    let middle = over + (over_length - length) / 2;
    let last = area + (area_length - length).max(0);
    middle.clamp(area, last)
}

impl Windows {
    pub(crate) fn dialog_spot(
        &self,
        parent: WindowId,
        size: Size<i32, Logical>,
    ) -> Option<Point<i32, Logical>> {
        let record = self.desktop.window(parent)?;
        let over = record.geometry();
        let area = record
            .output()
            .map_or(over, |output| self.usable_area(output));
        Some(Point::new(
            centered(over.loc.x, over.size.w, size.w, area.loc.x, area.size.w),
            centered(over.loc.y, over.size.h, size.h, area.loc.y, area.size.h),
        ))
    }

    fn is_shown(&self, window: WindowId) -> bool {
        self.desktop
            .window(window)
            .is_some_and(|record| record.visibility().shown())
    }
}

impl WindowsRef<'_> {
    pub(crate) fn parent_window(&self, window: WindowId) -> Option<WindowId> {
        let desktop = &self.windows.desktop;
        match desktop.window(window)?.kind() {
            WindowKind::Xdg(toplevel) => self.toplevel_parent(toplevel),
            WindowKind::X11(x11) => {
                desktop.window_for_x11(self.x11.get()?.window(x11)?.transient_for?)
            }
        }
        .filter(|parent| *parent != window)
    }

    pub(crate) fn toplevel_parent(&self, toplevel: ToplevelId) -> Option<WindowId> {
        let parent = self.shell.toplevel(toplevel)?.parent()?;
        self.windows.desktop.window_for_toplevel(parent)
    }

    pub(crate) fn is_modal(&self, window: WindowId) -> bool {
        let Some(record) = self.windows.desktop.window(window) else {
            return false;
        };
        match record.kind() {
            WindowKind::Xdg(toplevel) => self
                .shell
                .toplevel(toplevel)
                .is_some_and(|state| state.modal()),
            WindowKind::X11(x11) => self
                .x11
                .get()
                .and_then(|xwm| xwm.window(x11))
                .is_some_and(|state| state.states.contains(WindowStates::MODAL)),
        }
    }

    pub(crate) fn modal_blocker(&self, window: WindowId) -> Option<WindowId> {
        let mut blocker = None;
        let mut at = window;
        for _ in 0..FAMILY_DEPTH {
            let Some(modal) = self.windows.desktop.stacking().rev().find(|dialog| {
                self.parent_window(*dialog) == Some(at)
                    && self.is_modal(*dialog)
                    && self.windows.is_shown(*dialog)
            }) else {
                break;
            };
            blocker = Some(modal);
            at = modal;
        }
        blocker
    }

    pub(crate) fn family_root(&self, window: WindowId) -> WindowId {
        let mut root = window;
        for _ in 0..FAMILY_DEPTH {
            match self.parent_window(root) {
                Some(parent) if parent != window => root = parent,
                _ => break,
            }
        }
        root
    }

    pub(crate) fn family_of(&self, window: WindowId) -> Vec<WindowId> {
        let mut family = vec![self.family_root(window)];
        let mut at = 0;
        while at < family.len() && family.len() < FAMILY_DEPTH * FAMILY_DEPTH {
            for dialog in self.dialogs_of(family[at]) {
                if !family.contains(&dialog) {
                    family.push(dialog);
                }
            }
            at += 1;
        }
        family
    }

    pub(crate) fn dialogs_of(&self, parent: WindowId) -> Vec<WindowId> {
        self.windows
            .desktop
            .stacking()
            .filter(|window| self.parent_window(*window) == Some(parent))
            .collect()
    }
}
