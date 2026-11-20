use std::sync::Arc;

use browser_store::{ClosedTab, Place, PlaceId, ProfileId, SpaceId, Store};
use rikka_app::prelude::*;

use crate::home::clock::Minute;
use browser_model::TabId;

const CLOCK: &str = "rikka-browser-home-clock";
const SHELF: &str = "rikka-browser-home-shelf";
const SITES_READ: usize = 24;
const CLOSED_SHOWN: usize = 5;

#[derive(Debug, Default)]
struct Shelf {
    sites: Vec<Place>,
    closed: Vec<ClosedTab>,
}

#[derive(State)]
pub(crate) struct HomePage {
    #[untracked]
    shown_in: Option<TabId>,
    #[untracked]
    store: Option<Store>,
    #[untracked]
    profile: Option<ProfileId>,
    #[untracked]
    spaces: Vec<SpaceId>,
    now: Tracked<Minute>,
    sites: Tracked<Arc<[Place]>>,
    closed: Tracked<Arc<[ClosedTab]>>,
}

impl HomePage {
    pub(crate) fn new() -> HomePage {
        HomePage {
            shown_in: None,
            store: None,
            profile: None,
            spaces: Vec::new(),
            now: Tracked::new(Minute::now().0),
            sites: Tracked::new(Arc::from([])),
            closed: Tracked::new(Arc::from([])),
        }
    }

    pub(crate) fn shown_in(&self) -> Option<TabId> {
        self.shown_in
    }

    pub(crate) fn open(
        &mut self,
        tab: TabId,
        store: Store,
        profile: Option<ProfileId>,
        spaces: Vec<SpaceId>,
    ) {
        self.shown_in = Some(tab);
        self.store = Some(store);
        self.profile = profile;
        self.spaces = spaces;
        self.tick();
        self.refresh();
    }

    pub(crate) fn close(&mut self) {
        self.shown_in = None;
        cancel(CLOCK);
        cancel(SHELF);
    }

    pub(crate) fn now(&self) -> Minute {
        *self.now
    }

    pub(crate) fn sites(&self) -> &Arc<[Place]> {
        &self.sites
    }

    pub(crate) fn closed(&self) -> &Arc<[ClosedTab]> {
        &self.closed
    }

    pub(crate) fn site(&self, place: PlaceId) -> Option<&Place> {
        self.sites.iter().find(|site| site.id == place)
    }

    pub(crate) fn hide(&mut self, url: &str) {
        let (Some(store), Some(profile)) = (&self.store, self.profile) else {
            return;
        };
        store.hide_top_site(profile, url);
        self.refresh();
    }

    fn tick(&mut self) {
        let (now, next) = Minute::now();
        self.now.set(now);
        after_keyed(CLOCK, next, |page: &mut HomePage| page.tick());
    }

    fn refresh(&mut self) {
        let (Some(store), Some(profile)) = (self.store.clone(), self.profile) else {
            self.loaded(Shelf::default());
            return;
        };
        let spaces = self.spaces.clone();
        spawn_blocking_keyed(
            SHELF,
            move || read(&store, profile, &spaces),
            |page: &mut HomePage, shelf: browser_store::Result<Shelf>| match shelf {
                Ok(shelf) => page.loaded(shelf),
                Err(error) => tracing::warn!(%error, "failed to read home page data"),
            },
        );
    }

    fn loaded(&mut self, shelf: Shelf) {
        self.sites.set(Arc::from(shelf.sites));
        self.closed.set(Arc::from(shelf.closed));
    }
}

fn read(store: &Store, profile: ProfileId, spaces: &[SpaceId]) -> browser_store::Result<Shelf> {
    store.flush()?;
    let sites = store.top_sites(profile, SITES_READ)?;
    let closed = store
        .closed_tabs()?
        .into_iter()
        .filter(|closed| spaces.contains(&closed.space) && !closed.url.is_empty())
        .take(CLOSED_SHOWN)
        .collect();
    Ok(Shelf { sites, closed })
}

impl std::fmt::Debug for HomePage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HomePage")
            .field("shown_in", &self.shown_in)
            .field("sites", &self.sites.len())
            .field("closed", &self.closed.len())
            .finish_non_exhaustive()
    }
}
