use discord_core::Status;
use rikka_app::prelude::*;

use super::Chat;

const STATUS: &str = "discord-status";

impl Chat {
    pub(crate) fn set_status(&mut self, status: Status) {
        self.sidebar.close_menu();
        let Some(linked) = self.current() else {
            return;
        };
        let client = linked.client().clone();
        spawn_keyed(
            (STATUS, linked.key().to_owned()),
            async move { client.set_status(status).await },
            |chat: &mut Chat, result: discord_core::Result<()>| {
                if let Err(error) = result {
                    chat.error_message.set(Some(error.to_string()));
                }
            },
        );
    }

    pub(crate) fn muted(&self) -> bool {
        self.preferences.muted() || self.preferences.deafened()
    }

    pub(crate) fn deafened(&self) -> bool {
        self.preferences.deafened()
    }

    pub(crate) fn toggle_mute(&mut self) {
        if self.preferences.deafened() {
            self.preferences.set_deafened(false);
            self.preferences.set_muted(false);
        } else {
            let muted = self.preferences.muted();
            self.preferences.set_muted(!muted);
        }
        self.sync_engine();
    }

    pub(crate) fn toggle_deafen(&mut self) {
        let deafened = self.preferences.deafened();
        self.preferences.set_deafened(!deafened);
        self.sync_engine();
    }
}
