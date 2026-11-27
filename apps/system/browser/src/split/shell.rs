use browser_store::SplitGroupId;
use rikka_app::prelude::*;

use crate::shell::Shell;
use crate::split::drop_zones::Side;
use browser_model::Arrangement;
use browser_model::{Home, PinnedItem, Placement, Row, Slot, TabId, Target};

impl Shell {
    pub(crate) fn split_with_new_tab(&mut self) {
        let Some(anchor) = self.focused_tab() else {
            self.new_tab();
            return;
        };
        let opened =
            self.browser()
                .edit()
                .open_tab(self.id(), None, Placement::After(anchor), true);
        if let Some(opened) = opened {
            self.join_pane(anchor, opened, Arrangement::SideBySide, true);
        }
        self.changed();
        self.focus_address();
    }

    pub(crate) fn open_link_in_split(&mut self, anchor: TabId, url: String) {
        let opened =
            self.browser()
                .edit()
                .open_tab(self.id(), Some(url), Placement::After(anchor), true);
        if let Some(opened) = opened {
            self.join_pane(anchor, opened, Arrangement::SideBySide, true);
        }
        self.changed();
    }

    pub(crate) fn add_to_split(&mut self, tab: TabId) {
        let Some(anchor) = self.focused_tab() else {
            return;
        };
        if anchor == tab {
            self.split_with_new_tab();
            return;
        }
        if self.join_pane(anchor, tab, Arrangement::SideBySide, true) {
            self.select(tab);
        }
    }

    pub(crate) fn separate_pane(&mut self, tab: TabId) {
        if self.browser().edit().separate_pane(tab) {
            self.select(tab);
        }
    }

    pub(crate) fn dissolve_split(&mut self, group: SplitGroupId) {
        self.browser().edit().dissolve_split(group);
        self.changed();
    }

    pub(crate) fn arrange_split(&mut self, group: SplitGroupId, arrangement: Arrangement) {
        self.browser().edit().arrange_split(group, arrangement);
        self.schedule_save();
    }

    pub(crate) fn resize_split(&mut self, group: SplitGroupId, divider: usize, ratio: f32) {
        self.browser().edit().resize_split(group, divider, ratio);
        self.schedule_save();
    }

    pub(crate) fn focus_pane(&mut self, tab: TabId) {
        let placed = self
            .model()
            .window(self.id())
            .is_some_and(|layout| layout.holds(tab));
        if !placed || self.focused_tab() == Some(tab) {
            skip_view();
            return;
        }
        self.select(tab);
    }

    pub(crate) fn step_pane(&mut self, by: isize) {
        let next = self.model().pane_beside(self.id(), by);
        if let Some(next) = next {
            self.select(next);
        }
    }

    pub(crate) fn toggle_split(&mut self) {
        let shown = self.model().shown_split(self.id()).map(|group| group.id);
        match shown {
            Some(group) => self.dissolve_split(group),
            None => self.split_with_new_tab(),
        }
    }

    pub(crate) fn split_partners(&self, active: Option<TabId>) -> Vec<TabId> {
        let Some(active) = active else {
            return Vec::new();
        };
        self.model()
            .shown_split(self.id())
            .map(|group| {
                group
                    .panes()
                    .iter()
                    .copied()
                    .filter(|pane| *pane != active)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn pane_successor(&self, tab: TabId) -> Option<TabId> {
        let model = self.model();
        let layout = model.window(self.id())?;
        if layout.active() != Some(tab) {
            return None;
        }
        model.split_of(tab)?.successor(tab)
    }

    pub(crate) fn dropped_beside(&mut self, row: Row, anchor: TabId, side: Side) {
        let Some(tab) = self.pane_for(row, anchor) else {
            return;
        };
        let (along, after) = side.placement();
        if self.join_pane(anchor, tab, along, after) {
            self.select(tab);
        } else {
            self.changed();
        }
    }

    fn join_pane(&mut self, anchor: TabId, tab: TabId, along: Arrangement, after: bool) -> bool {
        let joined = self
            .browser()
            .edit()
            .split_beside(self.id(), anchor, tab, along, after);
        if joined {
            self.schedule_save();
        }
        joined
    }

    fn pane_for(&mut self, row: Row, anchor: TabId) -> Option<TabId> {
        let window = self.id();
        let mut model = self.browser().edit();
        let tab = match row {
            Row::Tab(tab) if model.window(window).is_some_and(|layout| layout.holds(tab)) => tab,
            Row::Tab(tab) => {
                let today = matches!(model.tab(tab)?.home, Home::Today(_));
                let at = model
                    .window(window)
                    .and_then(|layout| layout.place_in_today(anchor))
                    .map_or(0, |(_, at)| at + 1);
                if !today || !model.drop_row(window, Row::Tab(tab), Target::Today(at)) {
                    return None;
                }
                tab
            }
            Row::Pin(PinnedItem::Tab(pin)) => model.select_slot(window, Slot::Pinned(pin))?,
            Row::Pin(PinnedItem::Folder(_)) => return None,
            Row::Favorite(favorite) => {
                let url = model.favorites.find(favorite)?.1.saved.url.clone();
                model.open_tab(window, Some(url), Placement::After(anchor), true)?
            }
        };
        model.select(window, anchor);
        Some(tab)
    }

    fn focused_tab(&self) -> Option<TabId> {
        self.active_tab()
    }
}
