use browser_pages::Cause;
use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse, Filter};
use rikka_web::{NavigationButton, Session};

use crate::page::Page;
use crate::shell::Shell;
use browser_model::{Placement, TabId, same_zoom, zoom_step};
use browser_pages::Internal;

impl Shell {
    pub(crate) fn leave_address(&mut self) {
        self.address.end();
    }

    pub(crate) fn go(&mut self, destination: String, cause: Cause) {
        let active = self.active_tab();
        let tab = match active {
            Some(tab) => tab,
            None => {
                let opened = self
                    .browser
                    .edit()
                    .open_tab(self.id, None, Placement::Top, true);
                let Some(opened) = opened else {
                    return;
                };
                opened
            }
        };
        self.go_in(tab, destination, cause);
    }

    pub(crate) fn go_in(&mut self, tab: TabId, destination: String, cause: Cause) {
        self.navigate(tab, destination, cause);
        self.changed();
    }

    fn navigate(&mut self, tab: TabId, destination: String, cause: Cause) {
        let mut model = self.browser.edit();
        let Some(tab_entry) = model.tab_mut(tab) else {
            return;
        };
        if let Some(internal) = Internal::parse(&destination) {
            tab_entry.internal.set(Some(internal));
            return;
        }
        tab_entry.internal.set(None);
        tab_entry.visit_tracker.set_cause(cause);
        match tab_entry.page().view() {
            Some(view) => view.load(destination),
            None => {
                tab_entry.meta.url.set(destination);
                tab_entry.set_page(Page::Unloaded);
            }
        }
    }

    pub(crate) fn open_internal(&mut self, internal: Internal) {
        self.browser
            .edit()
            .open_tab(self.id, Some(internal.address()), Placement::Top, true);
        self.address.end();
        self.changed();
    }

    pub(crate) fn open_settings(&mut self) {
        self.open_internal(Internal::Settings(None));
    }

    pub(crate) fn back(&mut self) {
        let mut model = self.browser.edit();
        let Some(tab_entry) = model.active_mut(self.id) else {
            return;
        };
        if tab_entry.internal.is_some()
            && (tab_entry.page().live().is_some() || !tab_entry.meta.url.is_empty())
        {
            tab_entry.internal.set(None);
            drop(model);
            self.changed();
            return;
        }
        tab_entry.visit_tracker.set_cause(Cause::BackForward);
        if let Some(view) = tab_entry.page().view() {
            view.go_back();
        }
    }

    pub(crate) fn forward(&mut self) {
        if let Some(view) = self.active_session_with_cause(Cause::BackForward) {
            view.go_forward();
        }
    }

    pub(crate) fn go_to_history_index(&mut self, index: usize) {
        if let Some(view) = self.active_session_with_cause(Cause::BackForward) {
            view.go_to(index);
        }
    }

    pub(crate) fn navigation_button(&mut self, button: NavigationButton) {
        match button {
            NavigationButton::Back => self.back(),
            NavigationButton::Forward => self.forward(),
        }
    }

    pub(crate) fn reload(&mut self, bypassing_cache: bool) {
        match (
            self.active_session_with_cause(Cause::Reload),
            bypassing_cache,
        ) {
            (Some(view), true) => view.reload_bypassing_cache(),
            (Some(view), false) => view.reload(),
            (None, _) => {}
        }
    }

    pub(crate) fn stop(&mut self) {
        if let Some(view) = self.active_view() {
            view.stop();
        }
    }

    pub(crate) fn zoom(&mut self, by: i32) {
        let Some(view) = self.active_view() else {
            return;
        };
        let default = self.browser.preferences().default_zoom();
        let next = zoom_step(view.page().zoom, by, default);
        view.set_zoom(next);
        let mut model = self.browser.edit();
        if let Some(tab) = model.window(self.id).and_then(|layout| layout.active()) {
            model.remember_zoom(tab, next, default);
        }
    }

    pub(crate) fn fit_zoom(&mut self, tab: TabId) {
        let model = self.model();
        let Some(view) = model
            .tab(tab)
            .and_then(|tab_entry| tab_entry.page().view().cloned())
        else {
            return;
        };
        let default = self.browser.preferences().default_zoom();
        let zoom = model.zoom_for(tab, default);
        drop(model);
        if !same_zoom(view.page().zoom, zoom) {
            view.set_zoom(zoom);
        }
    }

    pub(crate) fn save_page(&mut self) {
        let Some((view, title)) = self.model().active(self.id).and_then(|tab_entry| {
            Some((
                tab_entry.page().view()?.clone(),
                (*tab_entry.meta.title).clone(),
            ))
        }) else {
            return;
        };
        let name = rikka_format::safe_file_name(&title.replace('/', "-"))
            .map_or_else(|| "page.pdf".to_owned(), |title| format!("{title}.pdf"));
        let dialog_title = tr!("browser.save.title").to_string();
        let parent = window_identifier();
        spawn_blocking(
            move || {
                Chooser::save(dialog_title)
                    .parent(parent)
                    .current_name(name)
                    .filter(Filter::named("PDF").pattern("*.pdf"))
                    .run()
            },
            move |_: &mut Shell, chosen: rikka_dialog::Result<ChooserResponse>| {
                let Ok(ChooserResponse::Files(files)) = chosen else {
                    return;
                };
                let Some(path) = files.into_iter().next() else {
                    return;
                };
                let view = view.clone();
                spawn(
                    async move { view.print_to_pdf(path).await },
                    |_: &mut Shell, saved: rikka_web::Result<()>| {
                        if let Err(error) = saved {
                            tracing::warn!(%error, "failed to save page");
                        }
                    },
                );
            },
        );
    }

    pub(crate) fn active_view(&self) -> Option<Session> {
        self.model()
            .active(self.id)
            .and_then(|tab_entry| tab_entry.page().view().cloned())
    }

    fn active_session_with_cause(&mut self, cause: Cause) -> Option<Session> {
        let mut model = self.browser.edit();
        let tab_entry = model.active_mut(self.id)?;
        let view = tab_entry.page().view()?.clone();
        tab_entry.visit_tracker.set_cause(cause);
        Some(view)
    }
}
