use browser_reader::{PROBE_SCRIPT, readable};
use rikka_app::prelude::*;
use rikka_web::World;
use serde_json::Value;

use crate::shell::Shell;
use browser_model::TabId;

pub(crate) const WORLD: &str = "rikka-reader";
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
        let uri = view.page().uri;
        if !(uri.starts_with("https://") || uri.starts_with("http://")) {
            return;
        }
        spawn_keyed(
            (PROBE, tab),
            async move { view.evaluate(PROBE_SCRIPT, &World::isolated(WORLD)).await },
            move |shell: &mut Shell, answer: rikka_web::Result<Value>| {
                skip_view();
                let offered = match answer {
                    Ok(answer) => readable(&answer).unwrap_or_else(|error| {
                        tracing::debug!(%error, "invalid reader probe result");
                        false
                    }),
                    Err(error) => {
                        tracing::debug!(%error, "reader probe failed");
                        false
                    }
                };
                shell.reader.offer(tab, offered);
            },
        );
    }
}
