use std::collections::HashMap;
use std::sync::Arc;

use browser_pages::VisitTracker;
use browser_store::Timestamp;
use rikka_ui::{Revision, Tracked};
use rikka_web::{CaptureState, PageState, Playback, Tooltip};

use crate::Favicon;
use crate::security::Security;

use crate::ids::{FavoriteId, PinnedTabId, ProfileId, SpaceId, TabId};
use crate::page::Page;
use crate::saved::Saved;
use browser_pages::Internal;

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Capturing {
    pub camera: bool,
    pub microphone: bool,
    pub screen: bool,
}

#[derive(Debug)]
pub struct Meta {
    pub url: Tracked<String>,
    pub title: Tracked<String>,
    pub favicon: Tracked<Option<Favicon>>,
    pub loading: Tracked<bool>,
    pub progress: Tracked<f32>,
    pub sound: Tracked<Sound>,
    pub capture: Tracked<Capturing>,
    pub security: Tracked<Security>,
    pub zoom: Tracked<f64>,
    pub back: Tracked<bool>,
    pub forward: Tracked<bool>,
    pub hovered: Tracked<Option<String>>,
    pub tooltip: Tracked<Option<Tooltip>>,
    seen_security: Option<(String, Arc<rikka_web::Security>)>,
}

impl Meta {
    fn from_saved(saved: Saved) -> Meta {
        Meta {
            url: Tracked::new(saved.url),
            title: Tracked::new(saved.title),
            favicon: Tracked::new(saved.favicon),
            loading: Tracked::new(false),
            progress: Tracked::new(0.0),
            sound: Tracked::new(Sound::Silent),
            capture: Tracked::new(Capturing::default()),
            security: Tracked::new(Security::Unknown),
            zoom: Tracked::new(1.0),
            back: Tracked::new(false),
            forward: Tracked::new(false),
            hovered: Tracked::new(None),
            tooltip: Tracked::new(None),
            seen_security: None,
        }
    }

    pub fn follow(&mut self, page: &PageState, muted: bool) {
        if !page.uri.is_empty() {
            self.url.set(page.uri.clone());
        }
        self.title.set(page.title.clone());
        self.loading.set(page.loading);
        self.progress.set(page.progress as f32);
        self.sound.set(match (page.media.playback, muted) {
            (_, true) => Sound::Muted,
            (Playback::Playing, false) => Sound::Playing,
            (Playback::None | Playback::Paused, false) => Sound::Silent,
        });
        self.capture.set(Capturing {
            camera: page.camera,
            microphone: page.microphone,
            screen: page.capture.screen != CaptureState::Off,
        });
        let seen = (page.uri.clone(), page.security.clone());
        if self.seen_security.as_ref() != Some(&seen) {
            self.security.set(Security::new(&seen.0, &seen.1));
            self.seen_security = Some(seen);
        }
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct CapturedState {
    url: String,
    bytes: Arc<[u8]>,
}

#[derive(Debug)]
pub struct Tab<L> {
    pub id: TabId,
    pub profile: ProfileKey,
    pub home: Home,
    page: Page<L>,
    shape: Revision,
    captured: Option<CapturedState>,
    pub last_active: Timestamp,
    pub visit_tracker: VisitTracker,
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
            captured: None,
            last_active: Timestamp::now(),
            visit_tracker: VisitTracker::default(),
            internal: Tracked::new(internal),
            meta: Meta::from_saved(saved),
        }
    }

    pub fn persistent_profile(&self) -> Option<ProfileId> {
        self.profile.persistent_id()
    }

    pub fn capture(&mut self, url: String, bytes: Arc<[u8]>) -> bool {
        if self
            .captured
            .as_ref()
            .is_some_and(|captured| captured.url == url && captured.bytes == bytes)
        {
            return false;
        }
        self.captured = Some(CapturedState { url, bytes });
        true
    }

    pub fn state(&self) -> Option<Arc<[u8]>> {
        match &self.page {
            Page::Discarded(bytes) => Some(Arc::clone(bytes)),
            Page::Live(_) => self
                .captured
                .as_ref()
                .filter(|captured| captured.url == *self.meta.url)
                .map(|captured| Arc::clone(&captured.bytes)),
            Page::Unloaded => None,
        }
    }

    pub fn touch(&mut self) {
        self.last_active = Timestamp::now();
    }

    pub fn page(&self) -> &Page<L> {
        self.shape.read();
        &self.page
    }

    pub fn set_page(&mut self, page: Page<L>) {
        let before = std::mem::replace(&mut self.page, page);
        self.captured = match (before, &self.page) {
            (Page::Discarded(bytes), Page::Live(_)) => Some(CapturedState {
                url: (*self.meta.url).clone(),
                bytes,
            }),
            (Page::Live(_), Page::Live(_)) => self.captured.take(),
            _ => None,
        };
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
