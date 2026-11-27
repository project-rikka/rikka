use rikka_app::prelude::*;
use rikka_web::{Event, Session};

use crate::shell::Shell;
use browser_model::TabId;

const WIRE: &str = "rikka-browser-tab";

impl Shell {
    pub(crate) fn sync_tabs(&mut self) {
        if self.close_if_emptied() {
            return;
        }
        let active = self.active_tab();
        let partners = self.split_partners(active);
        for tab in &partners {
            self.activate_tab(*tab);
        }
        if let Some(tab) = active {
            self.activate_tab(tab);
        }
        self.light(active);
        self.show(active, &partners);
        self.wire();
        self.prepare_page(active, &partners);
        self.follow_fullscreen(active);
        self.listen();
        self.prune_pages();
    }

    pub(crate) fn activate_tab(&mut self, tab: TabId) {
        match self.browser.activate(tab) {
            Ok(_) => self.engine_error.set(None),
            Err(error) => self.engine_error.set(Some(error.to_string())),
        }
    }

    fn light(&mut self, active: Option<TabId>) {
        if self.lit == active {
            return;
        }
        let mut model = self.browser.edit();
        for tab in [self.lit, active].into_iter().flatten() {
            if let Some(tab_entry) = model.tab_mut(tab) {
                tab_entry.touch();
            }
        }
        drop(model);
        self.lit = active;
        self.browser.mark_session_dirty();
    }

    fn show(&mut self, active: Option<TabId>, partners: &[TabId]) {
        let live =
            |shell: &Shell, tab: TabId| Some((tab, shell.model().tab(tab)?.page().view()?.clone()));
        let view = active.and_then(|tab| live(self, tab));
        let beside: Vec<(TabId, Session)> =
            partners.iter().filter_map(|tab| live(self, *tab)).collect();
        if view == self.shown && beside == self.beside {
            return;
        }
        let before: Vec<(TabId, Session)> = self
            .shown
            .take()
            .into_iter()
            .chain(std::mem::take(&mut self.beside))
            .collect();
        let after: Vec<&Session> = view.iter().chain(&beside).map(|(_, view)| view).collect();
        for (_, old) in &before {
            if !after.contains(&old) {
                old.set_visible(false);
            }
        }
        for shown in after {
            if !before.iter().any(|(_, old)| old == shown) {
                shown.set_visible(true);
                if shown.page().discarded {
                    shown.reload();
                }
            }
        }
        self.shown = view;
        self.beside = beside;
    }

    fn wire(&mut self) {
        let live: Vec<(TabId, Session)> = {
            let model = self.model();
            let Some(layout) = model.window(self.id) else {
                return;
            };
            layout
                .tabs()
                .into_iter()
                .filter_map(|tab| Some((tab, model.tab(tab)?.page().view()?.clone())))
                .collect()
        };
        self.wired.retain(|tab, view| {
            let still_live = live
                .iter()
                .any(|(candidate, live)| candidate == tab && live == view);
            if !still_live {
                cancel((WIRE, *tab));
            }
            still_live
        });
        for (tab, view) in live {
            if self.wired.contains_key(&tab) {
                continue;
            }
            let Some(events) = self
                .model()
                .tab(tab)
                .and_then(|tab_entry| Some(tab_entry.page().live()?.events().stream()))
            else {
                continue;
            };
            stream_keyed(
                (WIRE, tab),
                events,
                move |shell: &mut Shell, event: Event| {
                    shell.handle_event(tab, event);
                },
            );
            self.follow(tab, &view.page());
            self.wired.insert(tab, view);
        }
    }

    fn prune_pages(&mut self) {
        let browser = std::rc::Rc::clone(&self.browser);
        let model = browser.model();
        self.reader.retain(|tab| model.tab(tab).is_some());
        drop(model);
        self.prune_translations();
    }
}
