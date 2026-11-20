use rikka_compositor_desktop::LayerOrder;
use rikka_compositor_protocols::zwlr_layer_shell_v1::Layer as Level;
use rikka_compositor_protocols::zwlr_layer_surface_v1::KeyboardInteractivity;
use rikka_compositor_shell::{LayerEvent, LayerId};
use rikka_compositor_types::OutputId;
use rikka_session::surfaces;

use crate::context::{WindowsCtx, windows_ref};

impl WindowsCtx<'_> {
    fn place_layer(&mut self, layer: LayerId) {
        let Some(state) = self.shell.layer(layer) else {
            return;
        };
        let named = state
            .output()
            .and_then(|output| self.outputs.wayland().output_of(output));
        let surface = state.surface();
        let hides_windows =
            state.state().layer == Level::Overlay && surfaces::hides_windows(state.namespace());
        let is_scrim = surfaces::is_scrim(state.namespace());
        let order = if surfaces::is_input_panel(state.namespace()) {
            LayerOrder::Topmost
        } else {
            LayerOrder::Created
        };
        let layer_state = *state.state();
        let Some(output) = named.or_else(|| windows_ref!(self).active_output()) else {
            return self.shell.close_layer(layer);
        };
        if let Err(error) =
            self.windows
                .desktop
                .add_layer(layer, surface, output, layer_state, order)
        {
            tracing::warn!("cannot place layer surface: {error}");
            return self.shell.close_layer(layer);
        }
        if hides_windows {
            self.windows.overviews.push(layer);
            self.dismiss_snap_assist();
        }
        if is_scrim {
            self.windows.scrims.push(layer);
        }
    }

    pub(crate) fn arrange_layers(&mut self, output: OutputId) {
        let arrangement = match self.windows.desktop.arrange(output) {
            Ok(arrangement) => arrangement,
            Err(error) => return tracing::debug!("cannot arrange layer surfaces: {error}"),
        };
        for configure in arrangement.configures {
            self.shell.configure_layer(configure.layer, configure.size);
        }
        for layer in arrangement.closed {
            self.windows.forget_layer(layer);
            self.shell.close_layer(layer);
        }
        if arrangement.work_area_changed {
            self.work_area_changed(output);
            self.windows.session_changed();
        }
        self.frames.request_all();
    }

    pub(crate) fn layer_event(&mut self, layer: LayerId, event: LayerEvent) {
        match event {
            LayerEvent::Created => self.place_layer(layer),
            LayerEvent::InitialCommit | LayerEvent::Changed => self.restate_layer(layer),
            LayerEvent::Mapped => self.show_layer(layer, true),
            LayerEvent::Unmapped => self.show_layer(layer, false),
            LayerEvent::Configured { .. } => {}
            LayerEvent::Destroyed => self.remove_layer(layer),
        }
    }

    fn restate_layer(&mut self, layer: LayerId) {
        let Some(state) = self.shell.layer(layer).map(|state| *state.state()) else {
            return;
        };
        if self.windows.desktop.set_layer_state(layer, state).is_err() {
            return;
        }
        if let Some(output) = self
            .windows
            .desktop
            .layer(layer)
            .map(|layer| layer.output())
        {
            self.arrange_layers(output);
        }
        let unconfigured = self
            .shell
            .layer(layer)
            .is_some_and(|state| !state.initial_configure_sent());
        if unconfigured
            && let Some(size) = self
                .windows
                .desktop
                .layer(layer)
                .map(|layer| layer.geometry().size)
        {
            self.shell.configure_layer(layer, size);
        }
        self.keyboard_follows_layers(layer, false);
    }

    fn show_layer(&mut self, layer: LayerId, mapped: bool) {
        if self
            .windows
            .desktop
            .set_layer_mapped(layer, mapped)
            .is_err()
        {
            return;
        }
        if let Some(output) = self
            .windows
            .desktop
            .layer(layer)
            .map(|layer| layer.output())
        {
            self.arrange_layers(output);
        }
        self.keyboard_follows_layers(layer, mapped);
    }

    fn remove_layer(&mut self, layer: LayerId) {
        self.windows.forget_layer(layer);
        let Some(removed) = self.windows.desktop.remove_layer(layer) else {
            return;
        };
        self.arrange_layers(removed.output());
        self.release_layer_focus(removed.surface());
    }

    fn keyboard_follows_layers(&mut self, layer: LayerId, arrived: bool) {
        let Some(record) = self.windows.desktop.layer(layer) else {
            return;
        };
        let surface = record.surface();
        let mapped = record.mapped();
        let interactivity = record.keyboard_interactivity();
        let above_windows = matches!(record.level(), Level::Top | Level::Overlay);
        let focused = windows_ref!(self)
            .keyboard_focus()
            .is_some_and(|focus| focus.surface() == surface);
        let takes = mapped
            && match interactivity {
                KeyboardInteractivity::Exclusive => true,
                KeyboardInteractivity::OnDemand => {
                    arrived && above_windows && self.windows.exclusive_layer_surface().is_none()
                }
                _ => false,
            };
        if takes {
            if !focused && !self.windows.desktop.locked() {
                self.lend_keyboard(surface);
            }
        } else if focused && (!mapped || interactivity == KeyboardInteractivity::None) {
            self.release_layer_focus(surface);
        }
    }
}
