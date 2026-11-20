use std::time::Duration;

use rikka_compositor_geometry::{Logical, Size};
use rikka_compositor_shell::{
    DecorationMode, ToplevelConfigure, ToplevelId, ToplevelStates, WmCapabilities,
};
use rikka_wayland_wire::serial_not_before;

use super::state::State;
use super::views::{DesiredState, Gate};
use crate::view::ViewId;

const RESIZE_PATIENCE: Duration = Duration::from_millis(200);

const TILED: ToplevelStates = ToplevelStates::TILED_LEFT
    .union(ToplevelStates::TILED_RIGHT)
    .union(ToplevelStates::TILED_TOP)
    .union(ToplevelStates::TILED_BOTTOM);

fn configure_of(desired: &DesiredState) -> ToplevelConfigure {
    let mut states = TILED;
    states |= match desired.fullscreen {
        true => ToplevelStates::FULLSCREEN,
        false => ToplevelStates::MAXIMIZED,
    };
    if desired.focused {
        states |= ToplevelStates::ACTIVATED;
    }
    if !desired.visible {
        states |= ToplevelStates::SUSPENDED;
    }
    ToplevelConfigure {
        size: desired.size,
        states,
        bounds: desired.size,
        capabilities: Some(WmCapabilities::FULLSCREEN),
        decoration: Some(DecorationMode::ServerSide),
    }
}

impl State {
    pub(crate) fn resize(&mut self, view: ViewId, size: rikka_types::Size, scale: f64) {
        let size = Size::<i32, Logical>::new(
            size.width.round().max(1.0) as i32,
            size.height.round().max(1.0) as i32,
        );
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let Some(record) = self.record(view) else {
            return;
        };
        let rescaled = record.desired.scale != scale;
        record.desired.size = Some(size);
        record.desired.scale = scale;
        if rescaled {
            self.surfaces_follow(view);
        }
        self.describe_output();
        self.configure(view, false);
    }

    pub(crate) fn set_visible(&mut self, view: ViewId, visible: bool) {
        if let Some(record) = self.record(view) {
            record.desired.visible = visible;
        }
        self.configure(view, false);
    }

    pub(crate) fn set_fullscreen(&mut self, view: ViewId, fullscreen: bool) {
        if let Some(record) = self.record(view) {
            record.desired.fullscreen = fullscreen;
        }
        self.configure(view, false);
    }

    pub(crate) fn set_focused(&mut self, view: ViewId, focused: bool) {
        if let Some(record) = self.record(view) {
            record.desired.focused = focused;
        }
        self.configure(view, false);
    }

    pub(crate) fn close(&mut self, view: ViewId) {
        if let Some(toplevel) = self
            .views
            .records
            .get(&view)
            .and_then(|record| record.toplevel)
        {
            self.shell.close_toplevel(toplevel);
        }
    }

    pub(crate) fn toplevel_initial_commit(&mut self, toplevel: ToplevelId) {
        let view = self
            .views
            .toplevels
            .get(toplevel)
            .and_then(|tracked| tracked.view);
        match view {
            Some(view) => self.configure(view, true),
            None => {
                self.shell
                    .configure_toplevel(toplevel, configure_of(&DesiredState::default()));
            }
        }
    }

    pub(crate) fn toplevel_requested(&mut self, toplevel: ToplevelId) {
        let view = self
            .views
            .toplevels
            .get(toplevel)
            .and_then(|tracked| tracked.view);
        let initialized = self
            .shell
            .toplevel(toplevel)
            .is_some_and(|record| record.initial_configure_sent());
        match view {
            Some(view) => self.configure(view, true),
            None if initialized => {
                self.shell
                    .configure_toplevel(toplevel, configure_of(&DesiredState::default()));
            }
            None => {}
        }
    }

    pub(crate) fn configure(&mut self, view: ViewId, force: bool) {
        let Some(record) = self.views.records.get(&view) else {
            return;
        };
        let Some(toplevel) = record.toplevel else {
            return;
        };
        let Some(top) = self.shell.toplevel(toplevel) else {
            return;
        };
        if !top.initial_configure_sent() && !force {
            return;
        }
        let configure = configure_of(&record.desired);
        if !force && record.sent == Some(configure) {
            return;
        }
        let shown = self
            .window_geometry(toplevel)
            .map(|geometry| geometry.size)
            .filter(|_| top.mapped());
        let Some(serial) = self.shell.configure_toplevel(toplevel, configure) else {
            return;
        };
        let reshaped = shown.is_some_and(|shown| configure.size.is_some_and(|size| size != shown));
        let previous = self.record(view).and_then(|record| {
            record.sent = Some(configure);
            match reshaped {
                true => record.gate.take(),
                false => None,
            }
        });
        if let Some(previous) = previous {
            self.display.reactor().remove(previous.patience);
        }
        if reshaped {
            let patience =
                self.display
                    .reactor()
                    .after(RESIZE_PATIENCE, move |state: &mut State| {
                        state.open_gate(view, serial);
                    });
            if let Some(record) = self.record(view) {
                record.gate = Some(Gate { serial, patience });
            }
        }
    }

    pub(crate) fn toplevel_configured(&mut self, toplevel: ToplevelId, serial: u32) {
        let Some(view) = self
            .views
            .toplevels
            .get(toplevel)
            .and_then(|tracked| tracked.view)
        else {
            return;
        };
        let Some(gate) = self.record(view).and_then(|record| record.gate) else {
            return;
        };
        if serial_not_before(serial, gate.serial) {
            self.display.reactor().remove(gate.patience);
            self.open_gate(view, gate.serial);
        }
    }

    fn open_gate(&mut self, view: ViewId, serial: u32) {
        let Some(record) = self.record(view) else {
            return;
        };
        if record.gate.is_some_and(|gate| gate.serial == serial) {
            record.gate = None;
            record.dirty = true;
        }
    }
}
