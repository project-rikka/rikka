use rikka_app::prelude::*;

use crate::shell::Shell;
use browser_model::TabId;

const PROBE: &str = "rikka-browser-reader-probe";

impl Shell {
    pub(crate) fn reader_started(&mut self, tab: TabId) {
        cancel((PROBE, tab));
        if self.reader.forget(tab).is_some() {
            self.reader_closed(tab);
        }
    }

    pub(crate) fn probe_reader(&mut self, tab: TabId) {
        let Some(view) = self.view_of(tab) else {
            return;
        };
        let url = view.page().url;
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return;
        }
        spawn_keyed(
            (PROBE, tab),
            browser_reader::probe(&view),
            move |shell: &mut Shell, answer: browser_reader::Result<bool>| {
                skip_view();
                let offered = answer.unwrap_or_else(|error| {
                    tracing::debug!(%error, "reader probe failed");
                    false
                });
                shell.reader.offer(tab, offered);
            },
        );
    }
}
