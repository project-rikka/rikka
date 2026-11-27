use std::collections::HashMap;

use browser_pages::Internal;
use browser_store::Timestamp;
use rikka_ui::{Revision, Tracked};
use rikka_web::PageState;

use crate::Favicon;
use crate::ids::{FavoriteId, PinnedTabId, ProfileId, SpaceId, TabId};
use crate::page::Page;
use crate::saved::Saved;
use crate::security::Security;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProfileKey {
    Persistent(ProfileId),
    Private,
}

impl ProfileKey {
    pub fn persistent_id(self) -> Option<ProfileId> {
        match self {
            ProfileKey::Persistent(profile) => Some(profile),
            ProfileKey::Private => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Home {
    Today(Option<SpaceId>),
    Pinned(PinnedTabId),
    Favorite(FavoriteId),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Sound {
    #[default]
    Silent,
    Playing,
    Muted,
}

impl Sound {
    pub fn of(page: &PageState) -> Sound {
        match (page.muted, page.audible) {
            (true, _) => Sound::Muted,
            (false, true) => Sound::Playing,
            (false, false) => Sound::Silent,
        }
    }
}

#[derive(Debug)]
pub struct Meta {
    pub url: Tracked<String>,
    pub title: Tracked<String>,
    pub favicon: Tracked<Option<Favicon>>,
    pub loading: Tracked<bool>,
    pub sound: Tracked<Sound>,
    pub security: Tracked<Security>,
    pub zoom: Tracked<f64>,
    pub back: Tracked<bool>,
    pub forward: Tracked<bool>,
    pub hovered: Tracked<Option<String>>,
}

impl Meta {
    fn from_saved(saved: Saved) -> Meta {
        let security = Security::of(&saved.url);
        Meta {
            url: Tracked::new(saved.url),
            title: Tracked::new(saved.title),
            favicon: Tracked::new(saved.favicon),
            loading: Tracked::new(false),
            sound: Tracked::new(Sound::Silent),
            security: Tracked::new(security),
            zoom: Tracked::new(1.0),
            back: Tracked::new(false),
            forward: Tracked::new(false),
            hovered: Tracked::new(None),
        }
    }

    pub fn follow(&mut self, page: &PageState) {
        if !page.url.is_empty() {
            self.url.set(page.url.clone());
            self.security.set(Security::of(&page.url));
        }
        self.title.set(page.title.clone());
        self.loading.set(page.loading);
        self.sound.set(Sound::of(page));
        self.zoom.set(page.zoom);
        self.back.set(page.can_go_back);
        self.forward.set(page.can_go_forward);
        self.hovered.set(page.hovered_link.clone());
    }

    pub fn saved(&self) -> Saved {
        Saved {
            url: (*self.url).clone(),
            title: (*self.title).clone(),
            favicon: (*self.favicon).clone(),
        }
    }
}

#[derive(Debug)]
pub struct Tab<L> {
    pub id: TabId,
    pub profile: ProfileKey,
    pub home: Home,
    page: Page<L>,
    shape: Revision,
    pub last_active: Timestamp,
    pub internal: Tracked<Option<Internal>>,
    pub meta: Meta,
}

impl<L> Tab<L> {
    pub fn new(id: TabId, profile: ProfileKey, home: Home, saved: Saved) -> Tab<L> {
        let internal = Internal::parse(&saved.url);
        Tab {
            id,
            profile,
            home,
            page: Page::Unloaded,
            shape: Revision::new(),
            last_active: Timestamp::now(),
            internal: Tracked::new(internal),
            meta: Meta::from_saved(saved),
        }
    }

    pub fn persistent_profile(&self) -> Option<ProfileId> {
        self.profile.persistent_id()
    }

    pub fn touch(&mut self) {
        self.last_active = Timestamp::now();
    }

    pub fn page(&self) -> &Page<L> {
        self.shape.read();
        &self.page
    }

    pub fn set_page(&mut self, page: Page<L>) {
        self.page = page;
        self.shape.changed();
    }

    pub fn live_mut(&mut self) -> Option<&mut L> {
        self.page.live_mut()
    }

    pub fn address(&self) -> String {
        match &*self.internal {
            Some(internal) => internal.address(),
            None => (*self.meta.url).clone(),
        }
    }

    pub fn is_blank(&self) -> bool {
        self.internal.is_none() && self.meta.url.is_empty() && self.page().live().is_none()
    }

    pub fn shown_internal(&self) -> Option<Internal> {
        match &*self.internal {
            Some(internal) => Some(internal.clone()),
            None if self.is_blank() => Some(Internal::Home),
            None => None,
        }
    }

    pub fn shows_home(&self) -> bool {
        matches!(&*self.internal, Some(Internal::Home)) || self.is_blank()
    }

    pub fn saved(&self) -> Saved {
        Saved {
            url: self.address(),
            ..self.meta.saved()
        }
    }
}

#[derive(Debug)]
pub struct Tabs<L> {
    by_id: HashMap<TabId, Tab<L>>,
}

impl<L> Default for Tabs<L> {
    fn default() -> Tabs<L> {
        Tabs {
            by_id: HashMap::new(),
        }
    }
}

impl<L> Tabs<L> {
    pub fn get(&self, id: TabId) -> Option<&Tab<L>> {
        self.by_id.get(&id)
    }

    pub fn get_mut(&mut self, id: TabId) -> Option<&mut Tab<L>> {
        self.by_id.get_mut(&id)
    }

    pub fn insert(&mut self, tab: Tab<L>) {
        self.by_id.insert(tab.id, tab);
    }

    pub fn remove(&mut self, id: TabId) -> Option<Tab<L>> {
        self.by_id.remove(&id)
    }
}
