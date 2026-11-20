use rikka_seam::{Call, Seam};

use crate::context::{parts, person, windows_ctx};
use crate::input::compile_keymap;
use crate::policy::Policy;
use crate::settings::describe;
use crate::state::Compositor;

impl Compositor {
    pub(crate) fn answer(&mut self, call: Call) {
        Seam::dispatch_within(self, call, |compositor| compositor.session.seam.seam());
    }

    pub(crate) fn settings_changed(&mut self) {
        let Some(settings) = &mut self.settings else {
            return;
        };
        let rebuild = settings.refresh();
        if rebuild.keybindings {
            self.input
                .rebind(Some(&*settings), self.backend.switches_vt());
        }
        if rebuild.policy {
            let read = Policy::read(settings);
            self.policy_changed(read);
        }
        let Some(settings) = &mut self.settings else {
            return;
        };
        if rebuild.pointer && self.input.follow(settings.pointer_rules()) {
            self.backend.configure_devices(self.input.devices());
            let cursor = &self.input.rules().cursor;
            if self.cursor.set_theme(cursor.theme.as_deref(), cursor.size) {
                self.backend.cursor_changed(&mut parts!(self));
            }
        }
        if rebuild.displays {
            let displays = describe(self.outputs.iter());
            self.arrange(&displays);
        }
    }

    fn policy_changed(&mut self, read: Policy) {
        let keymap = self.policy.keymap().cloned();
        let looks = (self.policy.palette(), self.policy.caption());
        self.policy.follow(read);
        if (self.policy.palette(), self.policy.caption()) != looks {
            self.frames.request_all();
        }
        if self.scenes.set_decor(self.policy.decor()) {
            self.frames.request_all();
        }
        windows_ctx!(self).follow_animation_policy();
        let rules = self.policy.window_rules();
        windows_ctx!(self).apply_window_rules(rules);
        if !self.policy.minimizes_windows() {
            windows_ctx!(self).restore_every_minimized_window();
        }
        if self.policy.keymap() != keymap.as_ref() {
            match compile_keymap(self.policy.keymap()) {
                Ok(keymap) => {
                    person!(self).set_keymap(keymap);
                }
                Err(error) => tracing::warn!("keeping current keymap: {error}"),
            }
        }
    }
}
