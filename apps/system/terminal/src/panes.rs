use rikka_app::prelude::Axis;
use rikka_widgets::{Ratio, Side, SplitRatios};

use crate::group::Group;
use crate::tab::Tab;

const MIN_SHARE: f32 = 0.1;

#[derive(Debug)]
pub(crate) enum Panes {
    Leaf(Group),
    Split(Box<Split>),
}

#[derive(Debug)]
pub(crate) struct Split {
    id: u64,
    axis: Axis,
    ratio: Ratio,
    first: Panes,
    second: Panes,
}

impl Split {
    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    pub(crate) fn axis(&self) -> Axis {
        self.axis
    }

    pub(crate) fn ratio(&self) -> &Ratio {
        &self.ratio
    }

    pub(crate) fn first(&self) -> &Panes {
        &self.first
    }

    pub(crate) fn second(&self) -> &Panes {
        &self.second
    }
}

enum Walk {
    NotFound,
    Origin,
    Neighbour(u64),
}

impl Panes {
    pub(crate) fn is_split(&self) -> bool {
        matches!(self, Self::Split(_))
    }

    pub(crate) fn top_row(&self) -> Vec<(&Group, SplitRatios)> {
        let mut row = Vec::new();
        self.collect_top_row(SplitRatios::from(1.0), &mut row);
        row
    }

    fn collect_top_row<'a>(&'a self, share: SplitRatios, row: &mut Vec<(&'a Group, SplitRatios)>) {
        match self {
            Self::Leaf(group) => row.push((group, share)),
            Self::Split(split) if split.axis == Axis::Vertical => {
                split.first.collect_top_row(share, row);
            }
            Self::Split(split) => {
                split
                    .first
                    .collect_top_row(split.ratio.first().nested_in(share.clone()), row);
                split
                    .second
                    .collect_top_row(split.ratio.second().nested_in(share), row);
            }
        }
    }

    fn is_only(&self, group: u64) -> bool {
        matches!(self, Self::Leaf(only) if only.id() == group)
    }

    pub(crate) fn group(&self, id: u64) -> Option<&Group> {
        match self {
            Self::Leaf(group) => (group.id() == id).then_some(group),
            Self::Split(split) => split.first.group(id).or_else(|| split.second.group(id)),
        }
    }

    pub(crate) fn group_mut(&mut self, id: u64) -> Option<&mut Group> {
        match self {
            Self::Leaf(group) => (group.id() == id).then_some(group),
            Self::Split(split) => match split.first.group_mut(id) {
                Some(group) => Some(group),
                None => split.second.group_mut(id),
            },
        }
    }

    pub(crate) fn locate_tab(&self, tab: u64) -> Option<(u64, usize)> {
        match self {
            Self::Leaf(group) => group.position_of(tab).map(|index| (group.id(), index)),
            Self::Split(split) => split
                .first
                .locate_tab(tab)
                .or_else(|| split.second.locate_tab(tab)),
        }
    }

    pub(crate) fn tab_mut(&mut self, tab: u64) -> Option<&mut Tab> {
        match self {
            Self::Leaf(group) => {
                let index = group.position_of(tab)?;
                group.tab_mut(index)
            }
            Self::Split(split) => match split.first.tab_mut(tab) {
                Some(open_tab) => Some(open_tab),
                None => split.second.tab_mut(tab),
            },
        }
    }

    pub(crate) fn split(&mut self, target: u64, side: Side, id: u64, group: Group) {
        if let Self::Split(split) = self {
            match split.first.group(target).is_some() {
                true => split.first.split(target, side, id, group),
                false => split.second.split(target, side, id, group),
            }
            return;
        }
        if !self.is_only(target) {
            return;
        }
        let leaf = std::mem::take(self);
        let (first, second) = match side.after() {
            true => (leaf, Self::Leaf(group)),
            false => (Self::Leaf(group), leaf),
        };
        *self = Self::Split(Box::new(Split {
            id,
            axis: side.axis(),
            ratio: Ratio::new(0.5),
            first,
            second,
        }));
    }

    pub(crate) fn remove_group(&mut self, target: u64) -> bool {
        let Self::Split(split) = self else {
            return !self.is_only(target);
        };
        if !split.first.remove_group(target) {
            *self = std::mem::take(&mut split.second);
        } else if !split.second.remove_group(target) {
            *self = std::mem::take(&mut split.first);
        }
        true
    }

    pub(crate) fn resize(&mut self, id: u64, ratio: f32) {
        let Self::Split(split) = self else {
            return;
        };
        if split.id == id {
            split.ratio.set(ratio.clamp(0.0, 1.0));
            return;
        }
        split.first.resize(id, ratio);
        split.second.resize(id, ratio);
    }

    pub(crate) fn nudge(&mut self, group: u64, side: Side, step: f32) -> bool {
        let Self::Split(split) = self else {
            return false;
        };
        let inner = match split.first.group(group).is_some() {
            true => &mut split.first,
            false if split.second.group(group).is_some() => &mut split.second,
            false => return false,
        };
        if inner.nudge(group, side, step) {
            return true;
        }
        if split.axis != side.axis() {
            return false;
        }
        let delta = match side.after() {
            true => step,
            false => -step,
        };
        let ratio = (split.ratio.get() + delta).clamp(MIN_SHARE, 1.0 - MIN_SHARE);
        split.ratio.set(ratio);
        true
    }

    pub(crate) fn successor_of(&self, target: u64) -> Option<u64> {
        let Self::Split(split) = self else {
            return None;
        };
        if split.first.is_only(target) {
            return Some(split.second.nearest(side_toward(split.axis, true)));
        }
        if split.second.is_only(target) {
            return Some(split.first.nearest(side_toward(split.axis, false)));
        }
        split
            .first
            .successor_of(target)
            .or_else(|| split.second.successor_of(target))
    }

    pub(crate) fn neighbour(&self, from: u64, side: Side) -> Option<u64> {
        match self.find_neighbour(from, side) {
            Walk::Neighbour(group) => Some(group),
            Walk::NotFound | Walk::Origin => None,
        }
    }

    fn find_neighbour(&self, from: u64, side: Side) -> Walk {
        let split = match self {
            Self::Leaf(group) if group.id() == from => return Walk::Origin,
            Self::Leaf(_) => return Walk::NotFound,
            Self::Split(split) => split,
        };
        let across = split.axis == side.axis();
        match split.first.find_neighbour(from, side) {
            Walk::Neighbour(group) => Walk::Neighbour(group),
            Walk::Origin if across && side.after() => Walk::Neighbour(split.second.nearest(side)),
            Walk::Origin => Walk::Origin,
            Walk::NotFound => match split.second.find_neighbour(from, side) {
                Walk::Neighbour(group) => Walk::Neighbour(group),
                Walk::Origin if across && !side.after() => {
                    Walk::Neighbour(split.first.nearest(side))
                }
                Walk::Origin => Walk::Origin,
                Walk::NotFound => Walk::NotFound,
            },
        }
    }

    fn nearest(&self, side: Side) -> u64 {
        match self {
            Self::Leaf(group) => group.id(),
            Self::Split(split) if split.axis == side.axis() && !side.after() => {
                split.second.nearest(side)
            }
            Self::Split(split) => split.first.nearest(side),
        }
    }

    pub(crate) fn first(&self) -> u64 {
        match self {
            Self::Leaf(group) => group.id(),
            Self::Split(split) => split.first.first(),
        }
    }
}

fn side_toward(axis: Axis, after: bool) -> Side {
    match (axis, after) {
        (Axis::Horizontal, true) => Side::Right,
        (Axis::Horizontal, false) => Side::Left,
        (Axis::Vertical, true) => Side::Bottom,
        (Axis::Vertical, false) => Side::Top,
    }
}

impl Default for Panes {
    fn default() -> Self {
        Self::Leaf(Group::new(0))
    }
}
