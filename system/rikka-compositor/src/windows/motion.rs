use rikka_compositor_desktop::{WindowId, WindowKind};
use rikka_compositor_geometry::{Logical, Point, Rect};
use rikka_compositor_scrolling::Sizing;
use rikka_compositor_shell::ToplevelStates;
use rikka_compositor_types::OutputId;

use super::Enlargements;
use super::Windows;
use super::stretch::{Anchor, Stretch, gripped};
use crate::animation::Animations;
use crate::context::WindowsCtx;

impl Windows {
    pub(crate) fn resizing(&mut self, window: WindowId, resizing: bool) {
        let Some((record, policy)) = self.desktop.window_and_policy_mut(window) else {
            return;
        };
        if !matches!(record.kind(), WindowKind::Xdg(_)) {
            return;
        }
        let states = &mut policy.configure.states;
        if states.contains(ToplevelStates::RESIZING) != resizing {
            states.set(ToplevelStates::RESIZING, resizing);
            self.queue_configure(window);
        }
    }

    pub(crate) fn is_requesting(&self, window: WindowId, rect: Rect<i32, Logical>) -> bool {
        self.desktop
            .policy(window)
            .and_then(|policy| policy.stretch)
            .is_some_and(|stretch| stretch.is_requesting(rect))
    }
}

impl WindowsCtx<'_> {
    pub(crate) fn redraw(&mut self, output: OutputId) {
        match self.outputs.wayland().is_published(output) {
            true => self.frames.request(output),
            false => self.frames.request_all(),
        }
    }

    pub(crate) fn redraw_window(&mut self, window: WindowId) {
        match self.windows.output_of(window) {
            Some(output) => self.redraw(output),
            None => self.frames.request_all(),
        }
    }

    pub(crate) fn redraw_where_drawn(&mut self, windows: &[WindowId]) {
        if windows.is_empty() {
            return;
        }
        let desktop = &self.windows.desktop;
        let surfaces = &*self.surfaces;
        let outputs: Vec<OutputId> = desktop
            .outputs()
            .map(|(output, _)| output)
            .filter(|output| {
                desktop
                    .windows_drawn_on(surfaces, *output)
                    .any(|drawn| windows.contains(&drawn))
            })
            .collect();
        for output in outputs {
            self.redraw(output);
        }
    }

    pub(crate) fn travel(&mut self, window: WindowId, to: Rect<i32, Logical>) -> bool {
        self.travel_at(window, to, None)
    }

    pub(crate) fn travel_at(
        &mut self,
        window: WindowId,
        to: Rect<i32, Logical>,
        launch: Option<f64>,
    ) -> bool {
        let Some(from) = self
            .windows
            .desktop
            .window(window)
            .map(|record| record.geometry())
        else {
            return false;
        };
        if !self.stand(window, to) {
            return false;
        }
        self.animations.reshape(window, from.to_f64(), to, launch);
        true
    }

    pub(crate) fn ask_size(&mut self, window: WindowId, rect: Rect<i32, Logical>, anchor: Anchor) {
        let Some((record, policy)) = self.windows.desktop.window_and_policy_mut(window) else {
            return;
        };
        match record.kind() {
            WindowKind::Xdg(_) => {
                policy.configure.size = Some(rect.size);
                self.windows.queue_configure(window);
            }
            WindowKind::X11(_) => {
                if !self.configure_x11(window, rect) {
                    return;
                }
            }
        }
        self.stretch_to(window, rect, Some(anchor));
        self.windows.snapped_zone_moved(window, rect);
    }

    pub(crate) fn stretch_to(
        &mut self,
        window: WindowId,
        rect: Rect<i32, Logical>,
        anchor: Option<Anchor>,
    ) {
        if let Some(policy) = self.windows.desktop.policy_mut(window) {
            policy.stretch = Some(Stretch::requested(rect, anchor));
        }
        self.frames.request_all();
    }

    pub(crate) fn release_window(&mut self, window: WindowId) {
        if let Some(stretch) = self
            .windows
            .desktop
            .policy_mut(window)
            .and_then(|policy| policy.stretch.as_mut())
        {
            stretch.release();
        }
        self.frames.request_all();
    }

    pub(crate) fn release_all(&mut self) {
        let windows: Vec<WindowId> = self.windows.desktop.windows().map(|(id, _)| id).collect();
        for window in windows {
            self.release_window(window);
        }
    }

    pub(crate) fn stretched_committed(&mut self, window: WindowId) {
        let Some((record, policy)) = self.windows.desktop.window_and_policy_mut(window) else {
            return;
        };
        let Some(stretch) = policy.stretch else {
            return;
        };
        let geometry = record.geometry();
        if stretch.is_released() {
            policy.stretch = None;
        }
        if let Some(anchor) = stretch.anchor {
            let placed = anchor.place(geometry.size, geometry.loc);
            if placed != geometry.loc {
                self.move_window(window, placed);
            }
        }
    }

    pub(crate) fn restore_under_pointer(
        &mut self,
        window: WindowId,
        pointer: Point<f64, Logical>,
    ) -> Option<Point<i32, Logical>> {
        let tiled = self.windows.is_tiled(window);
        let (record, policy) = self.windows.desktop.window_and_policy_mut(window)?;
        let enlarged = policy.mode.enlarged()?;
        if enlarged.modes.fullscreen {
            return None;
        }
        let from = record.geometry();
        let to = Rect::new(
            gripped(from, enlarged.restore.size, pointer),
            enlarged.restore.size,
        );
        policy.mode = enlarged.shrunk(tiled);
        self.sync_x11_maximized(window, Enlargements::default());
        self.windows.tiling.enlarge(window, Sizing::Normal);
        if !self.stand(window, to) {
            return None;
        }
        self.animations.resize(window, from.size, to.size);
        self.windows.session_changed();
        Some(to.loc)
    }
}

impl Animations {
    pub(crate) fn finish_motion(&mut self, window: WindowId) {
        self.stop_travel(window);
    }
}
