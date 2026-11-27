use std::hash::Hash;

use rikka_app::prelude::*;
use rikka_web::ContextMenu;

use browser_model::{FavoriteId, FolderId, PinnedTabId, SpaceId, TabId};

pub(crate) const MARK: &str = "rikka-browser-menu-mark";
const BELOW: f32 = 4.0;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Subject {
    Tab(TabId),
    Pin(PinnedTabId),
    Folder(FolderId),
    Favorite(FavoriteId),
    TopSite(String),
    Space(SpaceId),
    Main,
    ClearHistory,
    Split,
    Page(TabId),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OpenMenu {
    pub(crate) subject: Subject,
    pub(crate) anchor: Anchor,
    pub(crate) key: u64,
}

#[derive(State)]
pub(crate) struct Menus {
    #[untracked]
    aimed: Option<Subject>,
    #[untracked]
    generation: u64,
    #[untracked]
    page: Option<ContextMenu>,
    shown: Tracked<Option<OpenMenu>>,
}

impl Menus {
    pub(crate) fn new() -> Menus {
        Menus {
            aimed: None,
            generation: 0,
            page: None,
            shown: Tracked::new(None),
        }
    }

    pub(crate) fn shown(&self) -> Option<&OpenMenu> {
        self.shown.as_ref()
    }

    pub(crate) fn page(&self) -> Option<&ContextMenu> {
        self.page.as_ref()
    }

    pub(crate) fn aim(&mut self, subject: Subject) {
        self.aimed = Some(subject);
    }

    pub(crate) fn open_at(&mut self, at: Point) {
        let Some(subject) = self.aimed.take() else {
            return;
        };
        let anchor = Anchor::new(MARK, AnchorEdge::BesideTop).offset(at.x, at.y);
        self.show(subject, anchor);
    }

    pub(crate) fn open_below(&mut self, subject: Subject, button: impl Hash) {
        self.show(subject, Anchor::below(button).offset(0.0, BELOW));
    }

    pub(crate) fn open_page(&mut self, tab: TabId, menu: ContextMenu, anchor: Anchor) {
        self.aimed = None;
        self.page = Some(menu);
        self.show(Subject::Page(tab), anchor);
    }

    pub(crate) fn close(&mut self) {
        self.aimed = None;
        self.page = None;
        if self.shown.is_some() {
            self.shown.set(None);
        }
    }

    fn show(&mut self, subject: Subject, anchor: Anchor) {
        self.generation += 1;
        self.shown.set(Some(OpenMenu {
            subject,
            anchor,
            key: self.generation,
        }));
    }
}

impl std::fmt::Debug for Menus {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Menus")
            .field("shown", &self.shown.as_ref().map(|shown| &shown.subject))
            .finish_non_exhaustive()
    }
}
