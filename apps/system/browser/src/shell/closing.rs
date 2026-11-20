use browser_store::Restored;
use rikka_app::prelude::*;

use crate::model::Closing;
use crate::shell::Shell;
use browser_model::{TabId, take_closed};

impl Shell {
    pub(crate) fn close_window(&mut self) {
        if !self.model().writes(self.id) {
            request_window(WindowRequest::Close);
            return;
        }
        let tabs = self
            .model()
            .window(self.id)
            .map(|layout| layout.tabs())
            .unwrap_or_default();
        self.gather(tabs, |_: &mut Shell| request_window(WindowRequest::Close));
    }

    pub(crate) fn quit(&mut self) {
        let tabs: Vec<TabId> = self
            .model()
            .windows
            .iter()
            .filter(|layout| !layout.private)
            .flat_map(|layout| layout.tabs())
            .collect();
        self.gather(tabs, Shell::close_all);
    }

    fn close_all(&mut self) {
        self.browser.freeze_session();
        for window in self.browser.windows() {
            if let Some(sender) = self.browser.sender(window) {
                let _ = sender.send(|_: &mut Shell| request_window(WindowRequest::Close));
            }
        }
    }

    pub(crate) fn save_sessions(&mut self, closing: impl IntoIterator<Item = Closing>) {
        for Closing { view, leaving } in closing {
            let state = view.state();
            let session = (!state.is_empty()).then(|| state.to_bytes());
            self.browser.edit().complete_leaving(leaving, session);
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
