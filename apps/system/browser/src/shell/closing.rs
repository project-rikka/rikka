use browser_store::Restored;
use rikka_app::prelude::*;
use rikka_web::Session;

use crate::shell::Shell;
use browser_model::take_closed;

impl Shell {
    pub(crate) fn close_window(&mut self) {
        request_window(WindowRequest::Close);
    }

    pub(crate) fn quit(&mut self) {
        if self.browser.quitting() {
            return;
        }
        for window in self.browser.begin_quit() {
            if let Some(sender) = self.browser.sender(window) {
                let _ = sender.send(Shell::ask_to_quit);
            }
        }
    }

    fn ask_to_quit(&mut self) {
        if self.browser.quitting() && self.browser.confirm_quit(self.id) {
            self.quit_confirmed();
        }
    }

    pub(crate) fn quit_confirmed(&mut self) {
        self.browser.cancel_quit();
        self.browser.freeze_session();
        for window in self.browser.windows() {
            if let Some(sender) = self.browser.sender(window) {
                let _ = sender.send(|_: &mut Shell| request_window(WindowRequest::Close));
            }
        }
    }

    pub(crate) fn close_views(&mut self, views: impl IntoIterator<Item = Session>) {
        for view in views {
            view.close();
        }
    }

    pub(crate) fn reopen(&mut self) {
        if self.private() {
            self.browser.edit().reopen_private(self.id);
            self.changed();
            return;
        }
        let store = self.browser.store();
        spawn_blocking(
            move || take_closed(&store),
            |shell: &mut Shell, taken: browser_store::Result<Option<Restored>>| match taken {
                Ok(Some(restored)) => shell.reopened(restored),
                Ok(None) => {}
                Err(error) => tracing::warn!(%error, "failed to reopen closed tab"),
            },
        );
    }

    pub(crate) fn reopened(&mut self, restored: Restored) {
        self.browser.edit().reopen_restored(self.id, restored);
        self.address.end();
        self.changed();
    }
}
