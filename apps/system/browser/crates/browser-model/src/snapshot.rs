use browser_store::{Session, SessionTab, SessionWindow, SpaceSession, WindowBounds};

use crate::page::LivePage;
use crate::{Home, Layout, Model, Slot, SpaceId, TabId};

impl<L: LivePage> Model<L> {
    pub fn session(&self) -> Session {
        Session {
            windows: self
                .windows
                .iter()
                .filter(|layout| !layout.private && !layout.little)
                .map(|layout| self.window_session(layout))
                .collect(),
        }
    }

    fn window_session(&self, layout: &Layout) -> SessionWindow {
        let mut spaces: Vec<SpaceSession> = Vec::new();
        for space in self.spaces.list() {
            let pinned = self
                .pinned
                .pins(space.id)
                .into_iter()
                .filter_map(|pin| layout.tab_in_slot(Slot::Pinned(pin)));
            let tabs: Vec<SessionTab> = pinned
                .chain(layout.today(Some(space.id)).iter().copied())
                .filter_map(|tab| self.tab_session(tab))
                .collect();
            if tabs.is_empty() {
                continue;
            }
            spaces.push(SpaceSession {
                space: space.id,
                active: self.shown_in(layout, space.id),
                tabs,
            });
        }
        SessionWindow {
            active_space: layout.space(),
            bounds: layout.geometry.map(|geometry| WindowBounds {
                position: None,
                width: geometry.width,
                height: geometry.height,
            }),
            maximized: layout.geometry.is_some_and(|geometry| geometry.maximized),
            spaces,
            splits: self.window_splits(layout.id),
        }
    }

    fn shown_in(&self, layout: &Layout, space: SpaceId) -> Option<TabId> {
        let active = layout.active().filter(|_| layout.space() == Some(space));
        active
            .or_else(|| layout.recent(Some(space)))
            .filter(|tab| layout.holds(*tab))
    }

    fn tab_session(&self, id: TabId) -> Option<SessionTab> {
        let tab = self.tab(id)?;
        let (pinned, favorite) = match tab.home {
            Home::Pinned(pin) => (Some(pin), None),
            Home::Favorite(favorite) => (None, Some(favorite)),
            Home::Today(_) => (None, None),
        };
        Some(SessionTab {
            id,
            pinned,
            favorite,
            url: tab.address(),
            title: (*tab.meta.title).clone(),
            last_active: tab.last_active,
            split: self.split_member(id),
        })
    }
}
