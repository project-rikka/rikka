use browser_store::{SessionSplit, SplitGroupId, SplitMember};

use crate::page::LivePage;
use crate::split::group::{Arrangement, Group};
use crate::split::saved;
use crate::{Home, Model, SpaceId, TabId, WindowId};

impl<L: LivePage> Model<L> {
    pub fn split_of(&self, tab: TabId) -> Option<&Group> {
        self.splits.containing(tab)
    }

    pub fn shown_split(&self, window: WindowId) -> Option<&Group> {
        let active = self.window(window)?.active()?;
        self.splits
            .containing(active)
            .filter(|group| group.window == window)
    }

    pub fn pane_space(&self, window: WindowId, tab: TabId) -> Option<Option<SpaceId>> {
        if !self.window(window)?.holds(tab) {
            return None;
        }
        match self.tab(tab)?.home {
            Home::Today(space) => Some(space),
            Home::Pinned(pin) => self.pinned.pin(pin).map(|(space, _)| Some(space)),
            Home::Favorite(_) => None,
        }
    }

    pub fn splittable_with(&self, window: WindowId, tab: TabId) -> bool {
        let Some(anchor) = self.window(window).and_then(|layout| layout.active()) else {
            return false;
        };
        if anchor == tab {
            return self.pane_space(window, tab).is_some()
                && self
                    .splits
                    .containing(tab)
                    .is_none_or(|group| !group.full());
        }
        let (Some(here), Some(there)) = (
            self.pane_space(window, anchor),
            self.pane_space(window, tab),
        ) else {
            return false;
        };
        here == there
            && self
                .splits
                .containing(anchor)
                .is_none_or(|group| !group.full() && !group.holds(tab))
    }

    pub fn split_candidates(&self, window: WindowId) -> Vec<TabId> {
        let Some(layout) = self.window(window) else {
            return Vec::new();
        };
        let Some(anchor) = layout.active() else {
            return Vec::new();
        };
        self.strip(window)
            .into_iter()
            .filter(|tab| *tab != anchor && self.splittable_with(window, *tab))
            .collect()
    }

    pub fn split_beside(
        &mut self,
        window: WindowId,
        anchor: TabId,
        tab: TabId,
        along: Arrangement,
        after: bool,
    ) -> bool {
        if anchor == tab {
            return false;
        }
        let (Some(space), Some(other)) = (
            self.pane_space(window, anchor),
            self.pane_space(window, tab),
        ) else {
            return false;
        };
        if space != other {
            return false;
        }
        let joined = match self.splits.containing(anchor).map(|group| group.id) {
            Some(id) => self.join_split(id, anchor, tab, after),
            None => {
                let panes = match after {
                    true => vec![anchor, tab],
                    false => vec![tab, anchor],
                };
                let id = self.store().new_split_group_id();
                self.splits.add(Group::new(id, window, space, along, panes));
                true
            }
        };
        if joined {
            self.splits.follow_focus(tab);
            self.gather_split(window, anchor);
        }
        joined
    }

    fn join_split(&mut self, id: SplitGroupId, anchor: TabId, tab: TabId, after: bool) -> bool {
        let Some(group) = self.splits.get(id) else {
            return false;
        };
        let moving = group.holds(tab);
        if !moving && group.full() {
            return false;
        }
        if !moving {
            self.splits.leave(tab);
        }
        self.splits
            .edit(id, |group| match moving {
                true => {
                    let at = group
                        .panes()
                        .iter()
                        .filter(|pane| **pane != tab)
                        .position(|pane| *pane == anchor)
                        .unwrap_or(0);
                    group.place(tab, at + usize::from(after))
                }
                false => {
                    let at = group.position(anchor).unwrap_or(0);
                    group.insert(at + usize::from(after), tab)
                }
            })
            .unwrap_or(false)
    }

    pub fn gather_split(&mut self, window: WindowId, around: TabId) {
        let Some(group) = self.splits.containing(around) else {
            return;
        };
        let (space, panes) = (group.space, group.panes().to_vec());
        let Some(layout) = self.windows.get(window) else {
            return;
        };
        let today = layout.today(space).to_vec();
        let members: Vec<TabId> = panes
            .iter()
            .copied()
            .filter(|pane| today.contains(pane))
            .collect();
        if members.len() < 2 {
            return;
        }
        let at = today
            .iter()
            .position(|tab| *tab == around)
            .or_else(|| today.iter().position(|tab| members.contains(tab)))
            .unwrap_or(0);
        let stays = |tab: &&TabId| !members.contains(tab);
        let order: Vec<TabId> = today[..at]
            .iter()
            .filter(stays)
            .chain(members.iter())
            .chain(today[at..].iter().filter(stays))
            .copied()
            .collect();
        if order != today
            && let Some(layout) = self.windows.get_mut(window)
        {
            layout.reorder_today(space, order);
        }
    }

    pub fn separate_pane(&mut self, tab: TabId) -> bool {
        self.splits.leave(tab)
    }

    pub fn dissolve_split(&mut self, id: SplitGroupId) {
        self.splits.dissolve(id);
    }

    pub fn resize_split(&mut self, id: SplitGroupId, divider: usize, ratio: f32) {
        self.splits.edit(id, |group| group.resize(divider, ratio));
    }

    pub fn arrange_split(&mut self, id: SplitGroupId, arrangement: Arrangement) {
        self.splits
            .edit(id, |group| group.arrangement = arrangement);
    }

    pub fn pane_beside(&self, window: WindowId, by: isize) -> Option<TabId> {
        let active = self.window(window)?.active()?;
        self.shown_split(window)?.step(active, by)
    }

    pub fn tidy_splits(&mut self) {
        let closed: Vec<TabId> = self
            .splits
            .all()
            .flat_map(|group| {
                group
                    .panes()
                    .iter()
                    .copied()
                    .filter(|tab| self.pane_space(group.window, *tab) != Some(group.space))
                    .collect::<Vec<_>>()
            })
            .collect();
        for tab in closed {
            self.splits.leave(tab);
        }
        let shown: Vec<TabId> = self
            .windows
            .iter()
            .filter_map(|layout| layout.active())
            .collect();
        for tab in shown {
            self.splits.follow_focus(tab);
        }
    }

    pub fn split_member(&self, tab: TabId) -> Option<SplitMember> {
        let group = self.splits.containing(tab)?;
        Some(saved::member(group, group.position(tab)?))
    }

    pub fn window_splits(&self, window: WindowId) -> Vec<SessionSplit> {
        self.splits
            .all()
            .filter(|group| group.window == window)
            .map(saved::split)
            .collect()
    }

    pub fn restore_splits(
        &mut self,
        window: WindowId,
        members: &[(TabId, SplitMember)],
        splits: &[SessionSplit],
    ) {
        for saved_group in saved::gathered(members, splits) {
            let Some(space) = saved_group
                .panes
                .first()
                .and_then(|tab| self.pane_space(window, *tab))
            else {
                continue;
            };
            let fits = saved_group.panes.len() >= 2
                && saved_group.panes.len() <= crate::split::group::MOST_PANES
                && saved_group
                    .panes
                    .iter()
                    .all(|tab| self.pane_space(window, *tab) == Some(space));
            if !fits || self.splits.get(saved_group.id).is_some() {
                continue;
            }
            let first = saved_group.panes[0];
            self.splits.add(
                Group::new(
                    saved_group.id,
                    window,
                    space,
                    saved_group.arrangement,
                    saved_group.panes,
                )
                .with_shares(&saved_group.shares),
            );
            self.gather_split(window, first);
        }
    }
}
