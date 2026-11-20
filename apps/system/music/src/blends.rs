use rikka_sync::RwLock;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use futures_util::Stream;
use music_blend::{AccountError, Blend, BlendId, Outside, Store, StoreSnapshot};
use music_core::{Id, Track};
use rikka_app::prelude::*;

type Alternatives = HashMap<(&'static str, Id), Vec<(&'static str, Id)>>;

#[derive(Clone, Default)]
pub(crate) struct Known(Arc<RwLock<Alternatives>>);

impl Known {
    pub(crate) fn alternatives(&self, track: &Track) -> Vec<(&'static str, Id)> {
        self.0
            .read()
            .get(&(track.service.slug, track.id.clone()))
            .cloned()
            .unwrap_or_default()
    }

    fn learn(&self, blends: &[Arc<Blend>]) {
        let mut learned = Alternatives::new();
        for entry in blends.iter().flat_map(|blend| blend.entries()) {
            let matched: Vec<(&'static str, Id)> = entry
                .found()
                .map(|(service, id)| (service, id.clone()))
                .collect();
            if !matched.is_empty() {
                let track = entry.track();
                learned.insert((track.service.slug, track.id.clone()), matched);
            }
        }
        *self.0.write() = learned;
    }
}

#[derive(State)]
pub(crate) struct Blends {
    #[untracked]
    store: Store,
    #[untracked]
    known: Known,
    #[untracked]
    discovering: bool,
    #[untracked]
    discovery_waits: bool,
    #[untracked]
    again: BTreeSet<BlendId>,
    listed: Tracked<Arc<[Arc<Blend>]>>,
    syncing: Tracked<Arc<BTreeSet<BlendId>>>,
    account_errors: Tracked<Arc<BTreeMap<BlendId, Arc<[AccountError]>>>>,
}

impl Blends {
    pub(crate) fn new(store: Store) -> Blends {
        let listed = listed(&store);
        let known = Known::default();
        known.learn(&listed);
        Blends {
            store,
            known,
            discovering: false,
            discovery_waits: false,
            again: BTreeSet::new(),
            listed: Tracked::new(listed),
            syncing: Tracked::default(),
            account_errors: Tracked::default(),
        }
    }

    pub(crate) fn known(&self) -> Known {
        self.known.clone()
    }

    pub(crate) fn listed(&self) -> &Arc<[Arc<Blend>]> {
        &self.listed
    }

    pub(crate) fn find(&self, id: &BlendId) -> Option<Arc<Blend>> {
        self.listed.iter().find(|blend| blend.id() == id).cloned()
    }

    pub(crate) fn ids(&self) -> Vec<BlendId> {
        self.listed.iter().map(|blend| blend.id().clone()).collect()
    }

    pub(crate) fn is_syncing(&self, id: &BlendId) -> bool {
        self.syncing.contains(id)
    }

    pub(crate) fn any_syncing(&self) -> bool {
        !self.syncing.is_empty()
    }

    pub(crate) fn begin_discovery(&mut self) -> bool {
        if self.discovering || !self.syncing.is_empty() {
            self.discovery_waits = true;
            return false;
        }
        self.discovering = true;
        self.discovery_waits = false;
        true
    }

    pub(crate) fn end_discovery(&mut self) {
        self.discovering = false;
    }

    pub(crate) fn begin_sync(&mut self, id: &BlendId) -> bool {
        if self.discovering {
            return false;
        }
        if self.syncing.contains(id) {
            self.again.insert(id.clone());
            return false;
        }
        self.set_syncing(id, true);
        true
    }

    pub(crate) fn end_sync(&mut self, id: &BlendId) -> bool {
        self.set_syncing(id, false);
        self.again.remove(id)
    }

    pub(crate) fn discovery_waits(&self) -> bool {
        self.discovery_waits && self.syncing.is_empty() && !self.discovering
    }

    pub(crate) fn account_errors_of(&self, id: &BlendId) -> Option<Arc<[AccountError]>> {
        self.account_errors.get(id).cloned()
    }

    pub(crate) fn mirrors(&self, service: &str, playlist: &Id) -> bool {
        self.listed
            .iter()
            .any(|blend| blend.mirrors_playlist(service, playlist))
    }

    pub(crate) fn snapshot(&self) -> StoreSnapshot {
        self.store.snapshot()
    }

    pub(crate) fn contains(&self, id: &BlendId) -> bool {
        self.store.find(id).is_some() || self.store.is_deleted(id)
    }

    pub(crate) fn failures(&self) -> impl Stream<Item = music_core::Error> + use<> {
        self.store.failures()
    }

    pub(crate) fn outside(&self) -> impl Stream<Item = Outside> + use<> {
        self.store.outside()
    }

    pub(crate) fn follow(&mut self, outside: Outside) {
        self.store.follow(outside);
        self.refresh();
    }

    pub(crate) fn save(&mut self, blend: Blend) {
        self.store.save(blend);
        self.refresh();
    }

    pub(crate) fn change(&mut self, id: &BlendId, change: impl FnOnce(&mut Blend) -> bool) -> bool {
        let Some(mut blend) = self.store.find(id).map(|stored| (**stored).clone()) else {
            return false;
        };
        if !change(&mut blend) {
            return false;
        }
        self.save(blend);
        true
    }

    pub(crate) fn delete(&mut self, id: &BlendId) {
        self.store.delete(id);
        self.again.remove(id);
        self.set_syncing(id, false);
        self.set_account_errors(id, Vec::new());
        self.refresh();
    }

    fn set_syncing(&mut self, id: &BlendId, syncing: bool) {
        if self.syncing.contains(id) == syncing {
            return;
        }
        let mut ids = (**self.syncing).clone();
        match syncing {
            true => ids.insert(id.clone()),
            false => ids.remove(id),
        };
        self.syncing.set(Arc::new(ids));
    }

    pub(crate) fn set_account_errors(&mut self, id: &BlendId, account_errors: Vec<AccountError>) {
        let known = self
            .account_errors
            .get(id)
            .map_or(&[][..], |errors| &errors[..]);
        if known == account_errors.as_slice() {
            return;
        }
        let mut by_blend = (**self.account_errors).clone();
        match account_errors.is_empty() {
            true => by_blend.remove(id),
            false => by_blend.insert(id.clone(), account_errors.into()),
        };
        self.account_errors.set(Arc::new(by_blend));
    }

    fn refresh(&mut self) {
        let listed = listed(&self.store);
        self.known.learn(&listed);
        self.listed.set(listed);
    }
}

fn listed(store: &Store) -> Arc<[Arc<Blend>]> {
    store.blends().into()
}

impl std::fmt::Debug for Blends {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Blends")
            .field("listed", &self.listed.len())
            .finish_non_exhaustive()
    }
}
