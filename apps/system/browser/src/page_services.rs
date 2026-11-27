use std::cell::{Ref, RefCell, RefMut};
use std::collections::{HashMap, HashSet};

use rikka_web::{Runtime, RuntimeEvent};

use crate::alerts::Alerts;
use crate::browser::Browser;
use crate::relay::WindowFeed;
use browser_model::Downloads;
use browser_model::{ProfileKey, WindowId};

#[derive(Debug, Default)]
pub(crate) struct PageServices {
    downloads: RefCell<Downloads>,
    snapshots: RefCell<HashSet<ProfileKey>>,
    alerts: RefCell<Alerts>,
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
    pub(crate) fn attach_runtime(&self, key: ProfileKey, runtime: &Runtime) {
        if let Some(events) = runtime.events() {
            self.feeds
                .borrow_mut()
                .runtimes
                .insert(key, WindowFeed::new(events));
        }
    }

    pub(crate) fn forget(&self, key: ProfileKey) {
        self.feeds.borrow_mut().runtimes.remove(&key);
        self.downloads.borrow_mut().forget(key);
        self.snapshots.borrow_mut().remove(&key);
    }

    pub(crate) fn first_snapshot(&self, key: ProfileKey) -> bool {
        self.snapshots.borrow_mut().insert(key)
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
