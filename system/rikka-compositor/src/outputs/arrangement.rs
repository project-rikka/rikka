use crate::context::parts;
use crate::settings::{Display, describe};
use crate::state::Compositor;

use super::anchor::anchor;
use super::{OutputConfig, OutputId};
use crate::context::input_ctx;

impl Compositor {
    pub(crate) fn displays_detected(&mut self) {
        input_ctx!(self).wake_displays();
        self.declare_and_arrange();
        self.announce_outputs();
        self.leasing.offer(&mut *self.backend, &self.core.display);
    }

    pub(crate) fn displays_resized(&mut self) {
        let displays = describe(self.outputs.iter());
        self.arrange(&displays);
        self.announce_outputs();
    }

    fn declare_and_arrange(&mut self) {
        let displays = describe(self.outputs.iter());
        if let Some(settings) = self
            .settings
            .as_mut()
            .filter(|settings| settings.declares_displays())
        {
            if !settings.declare_displays(&displays) {
                return;
            }
            tracing::info!(
                "{} display(s): {}",
                displays.len(),
                displays
                    .iter()
                    .map(|display| display.identity().label())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        self.arrange(&displays);
    }

    pub(crate) fn arrange(&mut self, displays: &[Display]) {
        let mut display_configs = match self
            .settings
            .as_ref()
            .filter(|settings| settings.declares_displays())
        {
            Some(settings) => settings.display_configs(displays),
            None => displays.iter().map(Display::automatic).collect(),
        };
        if !display_configs.is_empty() && display_configs.iter().all(|config| !config.enabled) {
            tracing::warn!("all outputs disabled; keeping the first one enabled");
            display_configs[0].enabled = true;
        }
        anchor(&mut display_configs);

        let configs: Vec<(OutputId, OutputConfig)> = displays
            .iter()
            .zip(display_configs)
            .filter(|(display, config)| {
                self.outputs
                    .get(display.id())
                    .and_then(|output| output.asked())
                    != Some(*config)
            })
            .map(|(display, config)| (display.id(), config))
            .collect();
        if configs.is_empty() {
            return;
        }
        if !self.configure_outputs(&configs) {
            tracing::warn!("cannot apply output configuration");
        }
    }

    pub(crate) fn configure_outputs(&mut self, configs: &[(OutputId, OutputConfig)]) -> bool {
        input_ctx!(self).wake_displays();
        let stays_lit = self.outputs.iter().any(|output| {
            configs
                .iter()
                .find(|(configured, _)| *configured == output.id())
                .map_or(output.enabled_config().is_some(), |(_, config)| {
                    config.enabled
                })
        });
        if !stays_lit {
            tracing::warn!("refusing to disable every output");
            return false;
        }
        let applied = self.backend.configure(&mut parts!(self), configs);
        if applied {
            for (output, config) in configs {
                self.outputs.ask(*output, *config);
            }
        }
        self.announce_outputs();
        applied
    }
}
