use std::collections::{HashMap, HashSet};

use crate::Favicon;
use browser_store::{Session, SessionWindow, Store, Timestamp};

use crate::ids::{FavoriteId, PinnedTabId, SpaceId, TabId};
use crate::saved::Saved;

#[derive(Debug, Default)]
pub struct Loaded {
    pub profiles: Vec<browser_store::Profile>,
    pub spaces: Vec<browser_store::Space>,
    pub favorites: Vec<browser_store::Favorite>,
    pub pinned: HashMap<SpaceId, Vec<browser_store::PinnedNode>>,
    pub session: Session,
    pub favicons: HashMap<String, Favicon>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Reopening {
    pub space: Option<SpaceId>,
    pub geometry: Option<crate::Geometry>,
    pub spaces: Vec<ReopeningSpace>,
    pub splits: Vec<browser_store::SessionSplit>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReopeningSpace {
    pub space: SpaceId,
    pub active: Option<TabId>,
    pub tabs: Vec<ReopeningTab>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReopeningTab {
    pub id: TabId,
    pub pinned: Option<PinnedTabId>,
    pub favorite: Option<FavoriteId>,
    pub saved: Saved,
    pub last_active: Timestamp,
    pub split: Option<browser_store::SplitMember>,
}

impl Loaded {
    pub fn read(store: &Store) -> browser_store::Result<Loaded> {
        let profiles = store.profiles()?;
        let spaces = store.spaces()?;
        let mut favorites = Vec::new();
        for profile in &profiles {
            favorites.extend(store.favorites(profile.id)?);
        }
        let mut pinned = HashMap::new();
        for space in &spaces {
            pinned.insert(space.id, store.pinned(space.id)?);
        }
        let session = store.load_session()?;
        let mut loaded = Loaded {
            profiles,
            spaces,
            favorites,
            pinned,
            session,
            favicons: HashMap::new(),
        };
        for url in loaded.urls() {
            match store.favicon_for_url(&url) {
                Ok(Some(image)) => {
                    let favicon = Favicon {
                        width: image.width,
                        height: image.height,
                        rgba: image.rgba,
                        svg: None,
                    };
                    loaded.favicons.insert(url, favicon);
                }
                Ok(None) => {}
                Err(error) => tracing::warn!(%error, "failed to read favicon"),
            }
        }
        Ok(loaded)
    }

    fn urls(&self) -> HashSet<String> {
        let mut urls: HashSet<String> = self
            .favorites
            .iter()
            .map(|favorite| favorite.url.clone())
            .collect();
        let mut stack: Vec<&browser_store::PinnedNode> = self.pinned.values().flatten().collect();
        while let Some(node) = stack.pop() {
            match node {
                browser_store::PinnedNode::Folder { children, .. } => stack.extend(children),
                browser_store::PinnedNode::Tab(tab) => {
                    urls.insert(tab.url.clone());
                }
            }
        }
        for window in &self.session.windows {
            for space in &window.spaces {
                urls.extend(space.tabs.iter().map(|tab| tab.url.clone()));
            }
        }
        urls
    }

    pub fn favicon(&self, url: &str) -> Option<Favicon> {
        self.favicons.get(url).cloned()
    }

    pub fn reopening(&self) -> Vec<Reopening> {
        self.session
            .windows
            .iter()
            .map(|window| self.reopening_window(window))
            .collect()
    }

    fn reopening_window(&self, window: &SessionWindow) -> Reopening {
        Reopening {
            space: window.active_space,
            geometry: window.bounds.map(|bounds| crate::Geometry {
                width: bounds.width,
                height: bounds.height,
                maximized: window.maximized,
            }),
            spaces: window
                .spaces
                .iter()
                .map(|space| ReopeningSpace {
                    space: space.space,
                    active: space.active,
                    tabs: space
                        .tabs
                        .iter()
                        .map(|tab| ReopeningTab {
                            id: tab.id,
                            pinned: tab.pinned,
                            favorite: tab.favorite,
                            saved: Saved {
                                url: tab.url.clone(),
                                title: tab.title.clone(),
                                favicon: self.favicon(&tab.url),
                            },
                            last_active: tab.last_active,
                            split: tab.split,
                        })
                        .collect(),
                })
                .collect(),
            splits: window.splits.clone(),
        }
    }
}
