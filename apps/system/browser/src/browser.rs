use std::cell::{Cell, Ref, RefCell, RefMut};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use browser_preferences::Preferences;
use browser_store::Store;
use rikka_app::Sender;
use rikka_web::{Runtime, Session};

use crate::engines::Engines;
use crate::model::Model;
use crate::page::{Live, Page};
use crate::services::Services;
use crate::shell::Shell;
use browser_model::{Loaded, ProfileId, ProfileKey, Reopening, TabId, WindowId};

#[derive(Debug)]
pub(crate) struct Browser {
    model: RefCell<Model>,
    engines: RefCell<Engines>,
    preferences: Preferences,
    senders: RefCell<HashMap<WindowId, Sender<Shell>>>,
    reopening: RefCell<Option<Vec<Reopening>>>,
    quitting: RefCell<Option<HashSet<WindowId>>>,
    focused: Cell<Option<WindowId>>,
    session: Keeping,
    services: Services,
}

#[derive(Debug, Default)]
struct Keeping {
    dirty: Cell<bool>,
    frozen: Cell<bool>,
}

impl Browser {
    pub(crate) fn new(preferences: Preferences, store: Store, loaded: &Loaded) -> Rc<Browser> {
        Rc::new(Browser {
            model: RefCell::new(Model::load(store, loaded, crate::model::NAMES)),
            engines: RefCell::new(Engines::default()),
            preferences,
            senders: RefCell::new(HashMap::new()),
            reopening: RefCell::new(Some(loaded.reopening())),
            quitting: RefCell::new(None),
            focused: Cell::new(None),
            session: Keeping::default(),
            services: Services::default(),
        })
    }

    pub(crate) fn model(&self) -> Ref<'_, Model> {
        self.model.borrow()
    }

    pub(crate) fn edit(&self) -> RefMut<'_, Model> {
        self.model.borrow_mut()
    }

    pub(crate) fn store(&self) -> Store {
        self.model().store().clone()
    }

    pub(crate) fn preferences(&self) -> &Preferences {
        &self.preferences
    }

    pub(crate) fn services(&self) -> &Services {
        &self.services
    }

    pub(crate) fn engine(&self, key: ProfileKey) -> rikka_web::Result<Runtime> {
        if let Some(runtime) = self.engines.borrow().running(key) {
            return Ok(runtime);
        }
        let runtime = self.engines.borrow_mut().start(key)?;
        self.services.chrome.started(key);
        self.services.page_services.attach_runtime(key, &runtime);
        for sender in self.senders.borrow().values() {
            let _ = sender.send(Shell::listen);
        }
        Ok(runtime)
    }

    pub(crate) fn running_engine(&self, key: ProfileKey) -> Option<Runtime> {
        self.engines.borrow().running(key)
    }

    pub(crate) fn forget_engine(&self, profile: ProfileId) {
        let key = ProfileKey::Persistent(profile);
        self.engines.borrow_mut().forget(profile);
        self.services.chrome.forget(key);
        self.services.page_services.forget(key);
    }

    pub(crate) fn history_changed(&self) {
        for sender in self.senders.borrow().values() {
            let _ = sender.send(Shell::history_changed);
        }
    }

    pub(crate) fn take_reopening(&self) -> Option<Vec<Reopening>> {
        self.reopening.borrow_mut().take()
    }

    pub(crate) fn join(&self, window: WindowId, sender: Sender<Shell>) {
        self.senders.borrow_mut().insert(window, sender);
        if self.model().writes(window) {
            self.session.frozen.set(false);
            self.session.dirty.set(true);
        }
    }

    pub(crate) fn focus(&self, window: WindowId, focused: bool) {
        match focused {
            true => self.focused.set(Some(window)),
            false if self.focused.get() == Some(window) => self.focused.set(None),
            false => {}
        }
    }

    pub(crate) fn focused(&self) -> Option<WindowId> {
        self.focused.get()
    }

    pub(crate) fn begin_quit(&self) -> Vec<WindowId> {
        let windows = self.windows();
        *self.quitting.borrow_mut() = Some(windows.iter().copied().collect());
        windows
    }

    pub(crate) fn quitting(&self) -> bool {
        self.quitting.borrow().is_some()
    }

    pub(crate) fn cancel_quit(&self) -> bool {
        self.quitting.borrow_mut().take().is_some()
    }

    pub(crate) fn confirm_quit(&self, window: WindowId) -> bool {
        let mut quitting = self.quitting.borrow_mut();
        let Some(waiting) = quitting.as_mut() else {
            return false;
        };
        waiting.remove(&window);
        waiting.is_empty()
    }

    pub(crate) fn leave(&self, window: WindowId) {
        self.senders.borrow_mut().remove(&window);
        self.focus(window, false);
        let agreed = self.confirm_quit(window);
        let last = {
            let model = self.model();
            model.writes(window)
                && !model
                    .windows
                    .iter()
                    .any(|layout| layout.id != window && model.writes(layout.id))
        };
        if last {
            self.save_session();
            self.session.frozen.set(true);
        }
        if !self.quitting() {
            self.close_views(window);
        }
        self.save_others(window);
        if agreed && let Some(sender) = self.windows().first().and_then(|id| self.sender(*id)) {
            let _ = sender.send(Shell::quit_confirmed);
        }
        let mut model = self.edit();
        model.remove_window(window);
        self.session.dirty.set(true);
        if !model.windows.iter().any(|layout| layout.private) {
            self.engines.borrow_mut().forget_private();
            self.services.chrome.forget(ProfileKey::Private);
            self.services.page_services.forget(ProfileKey::Private);
        }
    }

    fn close_views(&self, window: WindowId) {
        let model = self.model();
        let Some(layout) = model.window(window) else {
            return;
        };
        let views: Vec<Session> = layout
            .tabs()
            .into_iter()
            .filter_map(|tab| model.tab(tab)?.page().view().cloned())
            .collect();
        drop(model);
        for view in views {
            view.close();
        }
    }

    pub(crate) fn sender(&self, window: WindowId) -> Option<Sender<Shell>> {
        self.senders.borrow().get(&window).cloned()
    }

    pub(crate) fn windows(&self) -> Vec<WindowId> {
        self.senders.borrow().keys().copied().collect()
    }

    pub(crate) fn save_others(&self, except: WindowId) {
        let senders: Vec<Sender<Shell>> = self
            .senders
            .borrow()
            .iter()
            .filter(|(window, _)| **window != except)
            .map(|(_, sender)| sender.clone())
            .collect();
        for sender in senders {
            let _ = sender.send(Shell::sync_tabs);
        }
    }

    pub(crate) fn mark_session_dirty(&self) {
        self.session.dirty.set(true);
    }

    pub(crate) fn save_session(&self) {
        if self.session.frozen.get() || !self.session.dirty.replace(false) {
            return;
        }
        let model = self.model();
        model.store().save_session(model.session());
    }

    pub(crate) fn freeze_session(&self) {
        self.session.dirty.set(true);
        self.save_session();
        self.session.frozen.set(true);
    }

    pub(crate) fn finish(&self) {
        if let Err(error) = self.model().store().flush() {
            tracing::error!(%error, "failed to save browsing data");
        }
    }

    pub(crate) fn activate(&self, tab: TabId) -> rikka_web::Result<Option<Session>> {
        let (key, address) = {
            let model = self.model();
            let Some(tab_entry) = model.tab(tab) else {
                return Ok(None);
            };
            if tab_entry.internal.is_some()
                || tab_entry.meta.url.is_empty()
                || tab_entry.page().live().is_some()
            {
                return Ok(None);
            }
            (tab_entry.profile, (*tab_entry.meta.url).clone())
        };
        let runtime = self.engine(key)?;
        let live = Live::open(&runtime, &address)?;
        let view = live.view.clone();
        if let Some(tab_entry) = self.edit().tab_mut(tab) {
            tab_entry.set_page(Page::Live(live));
        }
        Ok(Some(view))
    }
}
