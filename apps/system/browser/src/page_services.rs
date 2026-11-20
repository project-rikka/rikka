use std::cell::{Ref, RefCell, RefMut};
use std::collections::HashMap;

use browser_store::SiteSettings;
use rikka_web::{Runtime, RuntimeEvent};

use crate::alerts::Alerts;
use crate::browser::Browser;
use crate::internal::Sources;
use crate::relay::WindowFeed;
use crate::requests::{Requests, share_all};
use browser_model::Downloads;
use browser_model::{Grants, Scope};
use browser_model::{ProfileKey, WindowId};

#[derive(Debug, Default)]
pub(crate) struct PageServices {
    requests: RefCell<Requests>,
    grants: RefCell<Grants>,
    downloads: RefCell<Downloads>,
    alerts: RefCell<Alerts>,
    sources: RefCell<Sources>,
    feeds: RefCell<Feeds>,
}

#[derive(Debug, Default)]
struct Feeds {
    runtimes: HashMap<ProfileKey, WindowFeed<RuntimeEvent>>,
    alerts: Option<WindowFeed<rikka_notify::Event>>,
}

impl Browser {
    pub(crate) fn page_services(&self) -> &PageServices {
        &self.services().page_services
    }
}

impl PageServices {
    pub(crate) fn attach_runtime(
        &self,
        key: ProfileKey,
        runtime: &Runtime,
        sites: &[SiteSettings],
    ) {
        if let Some(events) = runtime.events() {
            self.feeds
                .borrow_mut()
                .runtimes
                .insert(key, WindowFeed::new(events));
        }
        let ProfileKey::Persistent(id) = key else {
            return;
        };
        let decisions = sites
            .iter()
            .flat_map(|site| site.permissions.iter().cloned())
            .collect();
        self.grants.borrow_mut().load(id, decisions);
        share_all(runtime, &self.grants.borrow(), Scope::Profile(id));
    }

    pub(crate) fn left(&self, window: WindowId) {
        self.grants.borrow_mut().forget_scope(Scope::Window(window));
    }

    pub(crate) fn forget(&self, key: ProfileKey) {
        self.feeds.borrow_mut().runtimes.remove(&key);
        self.downloads.borrow_mut().forget(key);
        self.sources.borrow_mut().forget(key);
        if let ProfileKey::Persistent(profile) = key {
            self.grants
                .borrow_mut()
                .forget_scope(Scope::Profile(profile));
        }
    }

    pub(crate) fn requests(&self) -> Ref<'_, Requests> {
        self.requests.borrow()
    }

    pub(crate) fn requests_mut(&self) -> RefMut<'_, Requests> {
        self.requests.borrow_mut()
    }

    pub(crate) fn grants(&self) -> Ref<'_, Grants> {
        self.grants.borrow()
    }

    pub(crate) fn grants_mut(&self) -> RefMut<'_, Grants> {
        self.grants.borrow_mut()
    }

    pub(crate) fn downloads(&self) -> Ref<'_, Downloads> {
        self.downloads.borrow()
    }

    pub(crate) fn downloads_mut(&self) -> RefMut<'_, Downloads> {
        self.downloads.borrow_mut()
    }

    pub(crate) fn alerts_mut(&self) -> RefMut<'_, Alerts> {
        self.alerts.borrow_mut()
    }

    pub(crate) fn sources(&self) -> Ref<'_, Sources> {
        self.sources.borrow()
    }

    pub(crate) fn sources_mut(&self) -> RefMut<'_, Sources> {
        self.sources.borrow_mut()
    }

    pub(crate) fn subscribe_runtimes(
        &self,
        window: WindowId,
        alive: &[WindowId],
    ) -> Vec<(
        ProfileKey,
        impl futures_util::Stream<Item = RuntimeEvent> + Send + 'static + use<>,
    )> {
        self.feeds
            .borrow_mut()
            .runtimes
            .iter_mut()
            .filter_map(|(key, feed)| Some((*key, feed.subscribe(window, alive)?)))
            .collect()
    }

    pub(crate) fn subscribe_alerts(
        &self,
        window: WindowId,
        alive: &[WindowId],
    ) -> Option<impl futures_util::Stream<Item = rikka_notify::Event> + Send + 'static + use<>>
    {
        let mut feeds = self.feeds.borrow_mut();
        let alerts = self.alerts.borrow();
        let feed = feeds
            .alerts
            .get_or_insert_with(|| WindowFeed::new(alerts.notifications().events()));
        feed.subscribe(window, alive)
    }
}
