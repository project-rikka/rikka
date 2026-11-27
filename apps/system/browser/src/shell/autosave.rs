use std::time::Duration;

use browser_store::Timestamp;
use rikka_app::prelude::*;

use crate::shell::Shell;

const SAVE: &str = "rikka-browser-save";
const WATCH: &str = "rikka-browser-watch";
const SAVE_DELAY: Duration = Duration::from_secs(2);
const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(60);

impl Shell {
    pub(crate) fn schedule_save(&mut self) {
        self.browser.mark_session_dirty();
        after_keyed(SAVE, SAVE_DELAY, |shell: &mut Shell| {
            shell.browser.save_session();
        });
    }

    pub(crate) fn start_autosave(&mut self) {
        self.archive_idle();
        every_keyed(WATCH, AUTOSAVE_INTERVAL, |shell: &mut Shell| {
            shell.archive_idle();
            shell.browser.save_session();
        });
    }

    pub(crate) fn archive_idle(&mut self) {
        let Some(idle) = self.browser.preferences().archive_after().idle() else {
            return;
        };
        let before = Timestamp::now().saturating_sub(idle);
        let closing = self.browser.edit().archive_idle(self.id, before);
        if closing.is_empty() {
            return;
        }
        self.close_views(closing);
        self.changed();
    }
}
