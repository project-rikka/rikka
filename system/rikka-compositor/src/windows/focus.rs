use std::time::Instant;

use rikka_compositor_desktop::{Target, WindowId, WindowKind};
use rikka_compositor_protocols::zwlr_layer_shell_v1::Layer as Level;
use rikka_compositor_protocols::zwlr_layer_surface_v1::KeyboardInteractivity;
use rikka_compositor_seat::KeyboardFocus;
use rikka_compositor_shell::{LayerId, ToplevelStates};
use rikka_compositor_surface::SurfaceId;
use rikka_compositor_types::OutputId;

use super::Windows;
use crate::context::{WindowsCtx, WindowsRef, input_ref, person, windows_ref};

impl Windows {
    pub(crate) fn exclusive_layer_surface(&self) -> Option<SurfaceId> {
        self.desktop
            .outputs()
            .find_map(|(output, _)| self.desktop.keyboard_layer(output))
            .and_then(|layer| self.desktop.layer(layer))
            .map(|layer| layer.surface())
    }

    fn layer_accepts_keyboard(&self, layer: LayerId) -> bool {
        self.desktop.layer(layer).is_some_and(|record| {
            record.mapped() && record.keyboard_interactivity() != KeyboardInteractivity::None
        })
    }
}

impl WindowsRef<'_> {
    pub(crate) fn keyboard_focus(&self) -> Option<KeyboardFocus> {
        self.seats.get(self.input.seat)?.keyboard().focus()
    }

    pub(crate) fn focused_window(&self) -> Option<WindowId> {
        match self.keyboard_focus()? {
            KeyboardFocus::X11 { window, .. } => self
                .windows
                .desktop
                .windows()
                .find(|(_, record)| {
                    matches!(record.kind(), WindowKind::X11(x11) if x11.raw() == window)
                })
                .map(|(id, _)| id),
            KeyboardFocus::Surface(surface) => self.windows.window_of(self.surfaces, surface),
        }
    }

    pub(crate) fn layer_holds_focus(&self) -> bool {
        let Some(focus) = self.keyboard_focus() else {
            return false;
        };
        let Some(Target::Layer(layer)) = self.windows.target_of(self.surfaces, focus.surface())
        else {
            return false;
        };
        self.windows.layer_accepts_keyboard(layer)
            && self
                .windows
                .desktop
                .layer(layer)
                .is_some_and(|record| matches!(record.level(), Level::Top | Level::Overlay))
    }
}

impl WindowsCtx<'_> {
    pub(crate) fn refresh_activation_states(&mut self) {
        let focused = windows_ref!(self).focused_window();
        let previous = self.windows.recency.current();
        self.windows.recency.focused(focused, Instant::now());
        if previous != focused {
            self.redraw_outlines(previous, focused);
        }
        let windows: Vec<WindowId> = self.windows.desktop.windows().map(|(id, _)| id).collect();
        for window in windows {
            let Some((record, policy)) = self.windows.desktop.window_and_policy_mut(window) else {
                continue;
            };
            if !matches!(record.kind(), WindowKind::Xdg(_)) {
                continue;
            }
            let configure = &mut policy.configure;
            let active = focused == Some(window);
            if configure.states.contains(ToplevelStates::ACTIVATED) != active {
                configure.states.set(ToplevelStates::ACTIVATED, active);
                self.windows.queue_configure(window);
            }
        }
        if let Some(focused) = focused {
            self.scroll_to(focused);
        }
        self.windows.session_changed();
    }

    pub(crate) fn redraw_outlines(
        &mut self,
        previous: Option<WindowId>,
        focused: Option<WindowId>,
    ) {
        let outputs: Vec<OutputId> = [previous, focused]
            .into_iter()
            .flatten()
            .filter(|window| self.windows.is_tiled(*window) || self.windows.is_decorated(*window))
            .filter_map(|window| self.windows.output_of(window))
            .collect();
        for output in outputs {
            self.redraw(output);
        }
    }

    pub(crate) fn focus_window(&mut self, window: Option<WindowId>) {
        if self.windows.desktop.locked() {
            return;
        }
        let window =
            window.map(|window| windows_ref!(self).modal_blocker(window).unwrap_or(window));
        if let Some(window) = window {
            self.raise_family(window);
        }
        self.give_keyboard(window);
    }

    pub(crate) fn set_keyboard_window(&mut self, window: Option<WindowId>) {
        if self.windows.desktop.locked() {
            return;
        }
        let window =
            window.map(|window| windows_ref!(self).modal_blocker(window).unwrap_or(window));
        self.give_keyboard(window);
    }

    fn give_keyboard(&mut self, window: Option<WindowId>) {
        let record = window.and_then(|window| self.windows.desktop.window(window));
        let x11 = record.and_then(|record| match record.kind() {
            WindowKind::X11(x11) => Some(x11),
            WindowKind::Xdg(_) => None,
        });
        let focus = record.and_then(|record| {
            let surface = record.surface()?;
            Some(match x11 {
                Some(x11) => KeyboardFocus::X11 {
                    window: x11.raw(),
                    surface,
                },
                None => KeyboardFocus::Surface(surface),
            })
        });
        self.x11.focus(x11);
        self.set_keyboard_focus(focus);
    }

    pub(crate) fn set_keyboard_focus(&mut self, focus: Option<KeyboardFocus>) {
        person!(self).set_keyboard_focus(focus);
        self.refresh_activation_states();
    }

    pub(crate) fn focus_surface(&mut self, surface: SurfaceId) {
        self.set_keyboard_focus(Some(KeyboardFocus::Surface(surface)));
    }

    pub(crate) fn refocus(&mut self, on: Option<OutputId>) {
        if self.windows.desktop.locked() {
            return self.focus_lock_surface();
        }
        if let Some(surface) = self.windows.exclusive_layer_surface() {
            return self.focus_surface(surface);
        }
        if windows_ref!(self).layer_holds_focus() {
            return;
        }
        let on_display =
            |window: &WindowId| on.is_none_or(|on| self.windows.output_of(*window) == Some(on));
        let pointed = self
            .windows
            .rules()
            .mode
            .focus_follows_pointer()
            .then(|| input_ref!(self).pointed_window(self.cursor.position()))
            .flatten()
            .filter(on_display);
        let next = pointed.or_else(|| {
            self.windows
                .desktop
                .stacking()
                .rev()
                .filter(|window| {
                    self.windows.desktop.window(*window).is_some_and(|record| {
                        record.visibility().shown() && record.surface().is_some()
                    })
                })
                .find(on_display)
                .map(|window| self.windows.tiling.tiled_representative(window))
        });
        self.focus_window(next);
    }

    pub(crate) fn lend_keyboard(&mut self, surface: SurfaceId) {
        let from = windows_ref!(self).keyboard_focus();
        self.windows.focus_loans.lend(surface, from);
        self.focus_surface(surface);
    }

    pub(crate) fn release_layer_focus(&mut self, surface: SurfaceId) {
        let back = self.windows.focus_loans.repay(surface);
        let focused = windows_ref!(self)
            .keyboard_focus()
            .is_some_and(|focus| focus.surface() == surface);
        if !focused {
            return;
        }
        if !back.is_some_and(|back| self.give_back(back)) {
            self.refocus(None);
        }
    }

    fn give_back(&mut self, focus: KeyboardFocus) -> bool {
        if self.windows.desktop.locked() {
            return false;
        }
        let surface = focus.surface();
        match self.windows.target_of(self.surfaces, surface) {
            Some(Target::Window(window))
                if self
                    .windows
                    .desktop
                    .window(window)
                    .is_some_and(|record| record.visibility().shown()) =>
            {
                self.set_keyboard_window(Some(window));
                true
            }
            Some(Target::Layer(layer)) if self.windows.layer_accepts_keyboard(layer) => {
                self.focus_surface(surface);
                true
            }
            _ => false,
        }
    }
}
