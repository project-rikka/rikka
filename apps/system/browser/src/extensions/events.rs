use rikka_app::prelude::*;
use rikka_web::{
    ActionChange, Lifecycle, NewTabRequest, OpenWindowRequest, OptionsPageRequest, PopupOpening,
    RuntimeEvent, Session, TabCloseRequest, TabUpdateRequest,
};

use browser_pages::Cause;

use crate::extensions::service::{Ask, Asking};
use crate::page::Live;
use crate::shell::Shell;
use browser_model::{Placement, ProfileKey, TabId, WindowId};

const LISTING: &str = "rikka-browser-extensions";
const OPTIONS: &str = "rikka-browser-extension-options";

impl Shell {
    pub(crate) fn extension_event(&mut self, key: ProfileKey, event: RuntimeEvent) {
        let extensions = std::rc::Rc::clone(self.browser());
        let extensions = extensions.extensions();
        let asking = |ask: Ask| Asking {
            window: self.prompting_window(),
            key,
            ask,
        };
        match event {
            RuntimeEvent::Extension(change) => {
                let uninstalled = change.lifecycle == Lifecycle::Uninstalled;
                extensions.changed(key, change.extension, uninstalled);
            }
            RuntimeEvent::ExtensionInstall(request) => {
                extensions.fail(None);
                extensions.ask(asking(Ask::Install(request)));
            }
            RuntimeEvent::ExtensionUpdate(request) => {
                extensions.ask(asking(Ask::Update(request)));
            }
            RuntimeEvent::ExtensionPermissions(request) => {
                extensions.ask(asking(Ask::Permissions(request)));
            }
            RuntimeEvent::ExtensionInstallFailed(failure) => {
                let name = failure
                    .name
                    .or_else(|| {
                        failure
                            .extension
                            .and_then(|extension| extension.meta_data.name)
                    })
                    .or(failure.id)
                    .unwrap_or_default();
                extensions.fail(Some(name));
            }
            RuntimeEvent::ExtensionAction(change) if change.tab.is_none() => {
                extensions.act(key, (change.extension_id, change.kind), change.action);
            }
            RuntimeEvent::ExtensionTab(request) => self.extension_opens_tab(request),
            RuntimeEvent::ExtensionOptions(request) => self.extension_options_asked(key, request),
            unheard => drop(unheard),
        }
        self.browser().save_others(self.id());
    }

    fn prompting_window(&self) -> WindowId {
        self.browser().focused().unwrap_or(self.id())
    }

    pub(crate) fn tab_action_changed(&mut self, tab: TabId, change: ActionChange) {
        self.browser()
            .extensions()
            .act_on(tab, (change.extension_id, change.kind), change.action);
    }

    pub(crate) fn action_popup_opened(&mut self, tab: TabId, popup: PopupOpening) {
        let Some(uri) = popup.popup_uri else {
            return;
        };
        self.open_action_popup(tab, popup.extension_id, popup.kind, uri);
    }

    pub(crate) fn extension_updates_tab(&mut self, tab: TabId, request: TabUpdateRequest) {
        if let Some(url) = request.details.url.clone()
            && let Some(view) = self.view_of(tab)
        {
            view.load(url);
        }
        if request.details.active == Some(true) {
            self.select(tab);
        }
        request.complete();
    }

    pub(crate) fn extension_closes_tab(&mut self, tab: TabId, request: TabCloseRequest) {
        request.complete();
        self.close(tab);
    }

    fn adopt_in_other_window(&mut self, session: Session, address: String, focus: bool) {
        let target = self.prompting_window();
        let live = Live::adopt(session);
        let adopt = move |shell: &mut Shell| shell.adopt_page(live, address, focus);
        match target == self.id() {
            true => adopt(self),
            false => match self.browser().sender(target) {
                Some(sender) => self.raise_with(sender, adopt),
                None => adopt(self),
            },
        }
    }

    fn raise_with(
        &self,
        sender: rikka_app::Sender<Shell>,
        work: impl FnOnce(&mut Shell) + Send + 'static,
    ) {
        request_window(WindowRequest::Raise(sender.window()));
        let _ = sender.send(work);
    }

    pub(crate) fn adopt_page(&mut self, live: Live, address: String, focus: bool) {
        let key = self.model().profile_here(self.id());
        self.browser().dress(key, &live.view);
        let tab = {
            let mut model = self.browser().edit();
            let active = model.window(self.id()).and_then(|layout| layout.active());
            let placement = active.map_or(Placement::Top, Placement::After);
            let tab = model.adopt_tab(self.id(), live, address, placement, focus);
            if let Some(tab_entry) = tab.and_then(|tab| model.tab_mut(tab)) {
                tab_entry.visit_tracker.set_cause(Cause::Link);
                if !focus && let Some(view) = tab_entry.page().view() {
                    view.set_visible(false);
                }
            }
            tab
        };
        if tab.is_some() {
            self.changed();
        }
    }

    fn extension_opens_tab(&mut self, request: NewTabRequest) {
        let address = request.details.url.clone().unwrap_or_default();
        let focus = request.details.active.unwrap_or(true);
        match request.open(crate::page::settings()) {
            Ok(session) => self.adopt_in_other_window(session, address, focus),
            Err(error) => tracing::warn!(%error, "failed to open extension tab"),
        }
    }

    pub(crate) fn service_worker_opens(&mut self, request: OpenWindowRequest) {
        let address = request.uri.clone();
        match request.open(crate::page::settings()) {
            Ok(session) => {
                session.load(address.clone());
                self.adopt_in_other_window(session, address, true);
            }
            Err(error) => tracing::warn!(%error, "failed to open site window"),
        }
    }

    fn extension_options_asked(&mut self, key: ProfileKey, request: OptionsPageRequest) {
        let Some(runtime) = self.browser().running_engine(key) else {
            return;
        };
        let id = request.extension_id.clone();
        spawn_keyed(
            (OPTIONS, id.clone()),
            async move { (request, runtime.extensions().get(id).await) },
            move |shell: &mut Shell,
                  (request, found): (
                OptionsPageRequest,
                rikka_web::Result<rikka_web::Extension>,
            )| {
                match found.map(|extension| extension.meta_data.options_page_url) {
                    Ok(Some(url)) => {
                        request.complete();
                        shell.open_aside_focused(url);
                    }
                    Ok(None) => request.deny(),
                    Err(error) => {
                        tracing::warn!(%error, "extension options page not found");
                        request.deny();
                    }
                }
            },
        );
    }

    pub(crate) fn open_aside_focused(&mut self, url: String) {
        let opened = {
            let mut model = self.browser().edit();
            let active = model.window(self.id()).and_then(|layout| layout.active());
            let placement = active.map_or(Placement::Top, Placement::After);
            let opened = model.open_tab(self.id(), Some(url), placement, true);
            if let Some(tab_entry) = opened.and_then(|tab| model.tab_mut(tab)) {
                tab_entry.visit_tracker.set_cause(Cause::Link);
            }
            opened
        };
        if opened.is_some() {
            self.changed();
        }
    }

    pub(crate) fn list_extensions(&mut self) {
        let key = self.model().profile_here(self.id());
        let runtime = match self.browser().engine(key) {
            Ok(runtime) => runtime,
            Err(error) => {
                tracing::warn!(%error, "failed to list extensions");
                return;
            }
        };
        spawn_keyed(
            (LISTING, key),
            async move { runtime.extensions().list().await },
            move |shell: &mut Shell, listed: rikka_web::Result<Vec<rikka_web::Extension>>| {
                match listed {
                    Ok(listed) => shell.browser().extensions().listed(key, listed),
                    Err(error) => tracing::warn!(%error, "failed to list extensions"),
                }
            },
        );
    }
}
