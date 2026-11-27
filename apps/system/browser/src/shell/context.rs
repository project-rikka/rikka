use rikka_app::prelude::*;
use rikka_web::{ContextMenu, DownloadId, EditCommand};

use crate::shell::{Shell, WindowContent, window};
use browser_model::{Placement, TabId};

impl Shell {
    pub(crate) fn page_menu(&mut self, tab: TabId, menu: ContextMenu) {
        let anchor = crate::content::menu_anchor(tab, menu.at);
        self.menus.open_page(tab, menu, anchor);
    }

    pub(crate) fn open_link(&mut self, opener: TabId, url: String, focus: bool) {
        if self.little() {
            self.open_in_browser_window(url);
            return;
        }
        let opened =
            self.browser
                .edit()
                .open_tab(self.id, Some(url), Placement::After(opener), focus);
        if !focus && let Some(opened) = opened {
            match self.browser.activate(opened) {
                Ok(Some(view)) => view.set_visible(false),
                Ok(None) => {}
                Err(error) => tracing::warn!(%error, "failed to open a background tab"),
            }
        }
        self.changed();
    }

    pub(crate) fn open_link_in_window(&mut self, url: String, private: bool) {
        open_window(window(
            &self.browser,
            private,
            WindowContent::Addresses(vec![url]),
        ));
    }

    pub(crate) fn save_link(&mut self, tab: TabId, url: &str) {
        let Some(key) = self.model().tab(tab).map(|tab_entry| tab_entry.profile) else {
            return;
        };
        let runtime = match self.browser.engine(key) {
            Ok(runtime) => runtime,
            Err(error) => {
                tracing::warn!(%error, "failed to start Chrome for a download");
                return;
            }
        };
        spawn(
            runtime.downloads().start(url, true),
            |_: &mut Shell, started: rikka_web::Result<DownloadId>| {
                if let Err(error) = started {
                    tracing::warn!(%error, "failed to start a download");
                }
            },
        );
    }

    pub(crate) fn search_selection(&mut self, tab: TabId, terms: &str) {
        let url = self
            .browser
            .search_engines()
            .default_engine()
            .search_url(terms)
            .to_string();
        self.open_link(tab, url, true);
    }

    pub(crate) fn edit_page(&mut self, tab: TabId, frame: i64, command: EditCommand) {
        if let Some(view) = self.view_of(tab) {
            view.edit(frame, command);
        }
    }

    pub(crate) fn cut_page(&mut self, tab: TabId, frame: i64, selection: &str) {
        rikka_ui::clipboard::set_text(selection);
        self.edit_page(tab, frame, EditCommand::Delete);
    }

    pub(crate) fn paste_page(&mut self, tab: TabId, frame: i64) {
        rikka_ui::clipboard::read_text(move |shell: &mut Shell, text: Option<String>| {
            if let Some(text) = text {
                shell.edit_page(tab, frame, EditCommand::Paste(text));
            }
        });
    }

    pub(crate) fn page_back(&mut self, tab: TabId) {
        if let Some(view) = self.view_of(tab) {
            view.back();
        }
    }

    pub(crate) fn page_forward(&mut self, tab: TabId) {
        if let Some(view) = self.view_of(tab) {
            view.forward();
        }
    }

    pub(crate) fn page_reload(&mut self, tab: TabId) {
        if let Some(view) = self.view_of(tab) {
            view.reload();
        }
    }

    pub(crate) fn print_tab(&mut self, tab: TabId) {
        if let Some(view) = self.view_of(tab) {
            view.print();
        }
    }

    pub(crate) fn inspect(&mut self, tab: TabId) {
        if let Some(view) = self.view_of(tab) {
            view.inspect();
        }
    }
}
