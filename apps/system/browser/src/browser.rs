use std::cell::{Cell, Ref, RefCell, RefMut};
use std::collections::HashMap;
use std::rc::Rc;

use browser_pages::Cause;
use browser_store::{SiteSettings, Store, Timestamp};
use rikka_app::Sender;
use rikka_web::{Runtime, Session};

use crate::engines::Engines;
use crate::model::Model;
use crate::page::{Activation, Live, Page};
use crate::preferences::Preferences;
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
        let oldest = preferences
            .history_kept()
            .span()
            .map(|retention| Timestamp::now().saturating_sub(retention));
        store.fail_interrupted_downloads();
        for profile in &loaded.profiles {
            if let Some(oldest) = oldest {
                store.prune_history(profile.id, oldest);
            }
            store.decay_frecency_if_due(profile.id);
        }
        let services = Services::default();
        services
            .engine_settings
            .load(&store, loaded.profiles.iter().map(|profile| profile.id));
        Rc::new(Browser {
            model: RefCell::new(Model::load(store, loaded, crate::model::NAMES)),
            engines: RefCell::new(Engines::default()),
            preferences,
            senders: RefCell::new(HashMap::new()),
            reopening: RefCell::new(Some(loaded.reopening())),
            focused: Cell::new(None),
            session: Keeping::default(),
            services,
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
        let sites = self.site_settings(key);
        let preferences = self.web_preferences(key);
        let locales = self.web_locales(key);
        let runtime = self.engines.borrow_mut().start(key, |builder| {
            builder.preferences(preferences).locales(locales)
        })?;
        self.services
            .page_services
            .attach_runtime(key, &runtime, &sites);
        crate::https::allow_http_on(&runtime, &sites);
        Ok(runtime)
    }

    fn site_settings(&self, key: ProfileKey) -> Vec<SiteSettings> {
        let ProfileKey::Persistent(profile) = key else {
            return Vec::new();
        };
        self.store()
            .sites_with_settings(profile)
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "failed to read site settings");
                Vec::new()
            })
    }

    pub(crate) fn live_engines(&self) -> Vec<(ProfileKey, Runtime)> {
        self.engines.borrow().alive()
    }

    pub(crate) fn running_engine(&self, key: ProfileKey) -> Option<Runtime> {
        self.engines.borrow().running(key)
    }

    pub(crate) fn forget_engine(&self, profile: ProfileId) {
        let key = ProfileKey::Persistent(profile);
        self.engines.borrow_mut().forget(profile);
        self.services.extensions.forget(key);
        self.services.page_services.forget(key);
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

    pub(crate) fn leave(&self, window: WindowId) {
        self.senders.borrow_mut().remove(&window);
        self.focus(window, false);
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
        self.services.page_services.left(window);
        self.save_others(window);
        let mut model = self.edit();
        model.remove_window(window);
        self.session.dirty.set(true);
        if !model.windows.iter().any(|layout| layout.private) {
            self.engines.borrow_mut().forget_private();
            self.services.extensions.forget(ProfileKey::Private);
            self.services.page_services.forget(ProfileKey::Private);
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

    pub(crate) fn activate(&self, tab: TabId) -> rikka_web::Result<Option<(Session, Activation)>> {
        let (key, activation) = {
            let mut model = self.edit();
            let Some(tab_entry) = model.tab_mut(tab) else {
                return Ok(None);
            };
            if tab_entry.internal.is_some() {
                return Ok(None);
            }
            let address = (*tab_entry.meta.url).clone();
            let activation = match tab_entry.page() {
                Page::Live(_) => return Ok(None),
                Page::Discarded(session) => Activation::Restore {
                    session: session.clone(),
                    address,
                },
                Page::Unloaded if address.is_empty() => return Ok(None),
                Page::Unloaded => Activation::Load(address),
            };
            match activation {
                Activation::Restore { .. } => tab_entry.visit_tracker.set_cause(Cause::Restored),
                Activation::Load(_) => tab_entry.visit_tracker.set_cause_if_unset(Cause::Generated),
            }
            (tab_entry.profile, activation)
        };
        let runtime = self.engine(key)?;
        let live = Live::open(&runtime)?;
        self.dress(key, &live.view);
        let view = live.view.clone();
        if let Some(tab_entry) = self.edit().tab_mut(tab) {
            tab_entry.set_page(Page::Live(live));
        }
        Ok(Some((view, activation)))
    }
}
