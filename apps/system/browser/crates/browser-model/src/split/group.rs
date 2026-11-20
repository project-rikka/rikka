use browser_store::SplitGroupId;
use rikka_ui::Axis;
use rikka_ui::Observers;

use crate::{SpaceId, TabId, WindowId};

pub const MOST_PANES: usize = 4;
const MIN_SHARE: f32 = 0.08;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Arrangement {
    #[default]
    SideBySide,
    Stacked,
}

impl Arrangement {
    pub fn axis(self) -> Axis {
        match self {
            Arrangement::SideBySide => Axis::Horizontal,
            Arrangement::Stacked => Axis::Vertical,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub id: SplitGroupId,
    pub window: WindowId,
    pub space: Option<SpaceId>,
    pub arrangement: Arrangement,
    panes: Vec<TabId>,
    shares: Vec<f32>,
    focus: TabId,
}

impl Group {
    pub fn new(
        id: SplitGroupId,
        window: WindowId,
        space: Option<SpaceId>,
        arrangement: Arrangement,
        panes: Vec<TabId>,
    ) -> Group {
        let count = panes.len().max(1);
        let focus = panes[0];
        Group {
            id,
            window,
            space,
            arrangement,
            shares: vec![1.0 / count as f32; panes.len()],
            panes,
            focus,
        }
    }

    pub fn with_shares(mut self, shares: &[f32]) -> Group {
        if shares.len() == self.panes.len() && shares.iter().all(|share| *share > 0.0) {
            self.shares = shares.to_vec();
            self.normalize();
        }
        self
    }

    pub fn panes(&self) -> &[TabId] {
        &self.panes
    }

    pub fn shares(&self) -> &[f32] {
        &self.shares
    }

    pub fn focus(&self) -> TabId {
        self.focus
    }

    pub fn holds(&self, tab: TabId) -> bool {
        self.panes.contains(&tab)
    }

    pub fn position(&self, tab: TabId) -> Option<usize> {
        self.panes.iter().position(|pane| *pane == tab)
    }

    pub fn full(&self) -> bool {
        self.panes.len() >= MOST_PANES
    }

    pub fn ratio(&self, divider: usize) -> f32 {
        let rest: f32 = self.shares[divider..].iter().sum();
        match rest > f32::EPSILON {
            true => self.shares[divider] / rest,
            false => 0.5,
        }
    }

    pub fn resize(&mut self, divider: usize, ratio: f32) {
        if divider + 1 >= self.panes.len() {
            return;
        }
        let rest: f32 = self.shares[divider..].iter().sum();
        let after: f32 = self.shares[divider + 1..].iter().sum();
        let ratio = ratio.clamp(MIN_SHARE, 1.0 - MIN_SHARE);
        self.shares[divider] = rest * ratio;
        let scale = match after > f32::EPSILON {
            true => rest * (1.0 - ratio) / after,
            false => 1.0,
        };
        for share in &mut self.shares[divider + 1..] {
            *share *= scale;
        }
    }

    pub fn insert(&mut self, at: usize, tab: TabId) -> bool {
        if self.full() || self.holds(tab) {
            return false;
        }
        let count = self.panes.len() as f32;
        for share in &mut self.shares {
            *share *= count / (count + 1.0);
        }
        let at = at.min(self.panes.len());
        self.panes.insert(at, tab);
        self.shares.insert(at, 1.0 / (count + 1.0));
        true
    }

    pub fn place(&mut self, tab: TabId, at: usize) -> bool {
        let Some(from) = self.position(tab) else {
            return false;
        };
        let share = self.shares.remove(from);
        self.panes.remove(from);
        let at = at.min(self.panes.len());
        self.panes.insert(at, tab);
        self.shares.insert(at, share);
        true
    }

    pub fn remove(&mut self, tab: TabId) -> bool {
        let Some(at) = self.position(tab) else {
            return false;
        };
        self.panes.remove(at);
        self.shares.remove(at);
        self.normalize();
        if self.focus == tab
            && let Some(successor) = self
                .panes
                .get(at)
                .or_else(|| at.checked_sub(1).and_then(|before| self.panes.get(before)))
        {
            self.focus = *successor;
        }
        true
    }

    pub fn successor(&self, tab: TabId) -> Option<TabId> {
        let at = self.position(tab)?;
        self.panes
            .get(at + 1)
            .or_else(|| at.checked_sub(1).and_then(|before| self.panes.get(before)))
            .copied()
    }

    pub fn step(&self, from: TabId, by: isize) -> Option<TabId> {
        let at = self.position(from)? as isize;
        let count = self.panes.len() as isize;
        Some(self.panes[(at + by).rem_euclid(count) as usize])
    }

    pub fn set_focus(&mut self, tab: TabId) -> bool {
        if !self.holds(tab) || self.focus == tab {
            return false;
        }
        self.focus = tab;
        true
    }

    pub fn alive(&self) -> bool {
        self.panes.len() >= 2
    }

    fn normalize(&mut self) {
        let total: f32 = self.shares.iter().sum();
        if total <= f32::EPSILON {
            let count = self.shares.len().max(1) as f32;
            self.shares
                .iter_mut()
                .for_each(|share| *share = 1.0 / count);
            return;
        }
        self.shares.iter_mut().for_each(|share| *share /= total);
    }
}

#[derive(Debug, Default)]
pub struct Splits {
    groups: Vec<Group>,
    observers: Observers,
}

impl Splits {
    pub fn containing(&self, tab: TabId) -> Option<&Group> {
        self.observers.read();
        self.groups.iter().find(|group| group.holds(tab))
    }

    pub fn get(&self, id: SplitGroupId) -> Option<&Group> {
        self.observers.read();
        self.groups.iter().find(|group| group.id == id)
    }

    pub fn in_space(
        &self,
        window: WindowId,
        space: Option<SpaceId>,
    ) -> impl Iterator<Item = &Group> {
        self.observers.read();
        self.groups
            .iter()
            .filter(move |group| group.window == window && group.space == space)
    }

    pub fn add(&mut self, group: Group) {
        for tab in group.panes() {
            self.leave(*tab);
        }
        self.groups.push(group);
        self.observers.changed();
    }

    pub fn edit<T>(&mut self, id: SplitGroupId, edit: impl FnOnce(&mut Group) -> T) -> Option<T> {
        let group = self.groups.iter_mut().find(|group| group.id == id)?;
        let edited = edit(group);
        self.groups.retain(Group::alive);
        self.observers.changed();
        Some(edited)
    }

    pub fn leave(&mut self, tab: TabId) -> bool {
        let mut left = false;
        for group in &mut self.groups {
            left |= group.remove(tab);
        }
        if left {
            self.groups.retain(Group::alive);
            self.observers.changed();
        }
        left
    }

    pub fn dissolve(&mut self, id: SplitGroupId) -> Option<Group> {
        let at = self.groups.iter().position(|group| group.id == id)?;
        self.observers.changed();
        Some(self.groups.remove(at))
    }

    pub fn all(&self) -> impl Iterator<Item = &Group> {
        self.groups.iter()
    }

    pub fn follow_focus(&mut self, tab: TabId) {
        let mut changed = false;
        for group in &mut self.groups {
            changed |= group.set_focus(tab);
        }
        if changed {
            self.observers.changed();
        }
    }
}
