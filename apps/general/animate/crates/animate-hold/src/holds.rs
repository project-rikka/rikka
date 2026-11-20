use animate_body::Side;

use crate::hold::{Hold, Holder, MAX_EASE};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HoldId(u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HoldLane {
    Hand(Side),
    Pin(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaneHolds {
    pub lane: HoldLane,
    pub holds: Vec<HoldId>,
}

#[derive(Clone, Debug, PartialEq)]
struct Held {
    id: HoldId,
    hold: Hold,
}

#[derive(Clone, Debug, Default)]
pub struct Holds {
    holds: Vec<Held>,
    next: u32,
}

impl PartialEq for Holds {
    fn eq(&self, other: &Self) -> bool {
        self.holds == other.holds
    }
}

impl Holds {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.holds.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.holds.len()
    }

    #[must_use]
    pub fn get(&self, id: HoldId) -> Option<&Hold> {
        self.at(id).map(|at| &self.holds[at].hold)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Hold> {
        self.holds.iter().map(|held| &held.hold)
    }

    pub fn entries(&self) -> impl Iterator<Item = (HoldId, &Hold)> {
        self.holds.iter().map(|held| (held.id, &held.hold))
    }

    pub fn holding(&self, frame: u32) -> impl Iterator<Item = (HoldId, &Hold)> {
        self.entries().filter(move |(_, hold)| hold.holds_at(frame))
    }

    pub fn active(&self, frame: f32) -> impl Iterator<Item = (HoldId, &Hold, f32)> {
        self.entries().filter_map(move |(id, hold)| {
            let strength = hold.strength(frame);
            (strength > 0.0).then_some((id, hold, strength))
        })
    }

    #[must_use]
    pub fn closing(&self, side: Side, frame: f32) -> f32 {
        self.iter()
            .filter(|hold| hold.hand() == Some(side))
            .map(|hold| hold.strength(frame))
            .fold(0.0, f32::max)
    }

    #[must_use]
    pub fn last_frame(&self) -> u32 {
        self.iter()
            .map(|hold| hold.until.unwrap_or(hold.from).max(hold.from))
            .max()
            .unwrap_or(0)
    }

    #[must_use]
    pub fn changed_since(&self, before: &Holds) -> Option<u32> {
        let unmatched = |one: &Holds, other: &Holds| {
            one.iter()
                .filter(|hold| !other.iter().any(|known| known == *hold))
                .map(Hold::closes_at)
                .min()
        };
        [unmatched(self, before), unmatched(before, self)]
            .into_iter()
            .flatten()
            .min()
    }

    pub fn lay(&mut self, mut hold: Hold) -> HoldId {
        hold.ease = hold.ease.min(MAX_EASE);
        if let Some(side) = hold.hand() {
            self.holds
                .retain(|other| other.hold.hand() != Some(side) || other.hold.from != hold.from);
            for other in &mut self.holds {
                let other = &mut other.hold;
                if other.hand() != Some(side) {
                    continue;
                }
                if other.from < hold.from && other.holds_at(hold.from) {
                    other.until = Some(hold.from);
                } else if other.from > hold.from
                    && hold.until.is_none_or(|until| until > other.from)
                {
                    hold.until = Some(other.from);
                }
            }
        }
        let id = HoldId(self.next);
        self.next += 1;
        self.insert(Held { id, hold });
        id
    }

    pub fn release(&mut self, id: HoldId, frame: u32) {
        let Some(at) = self.at(id) else {
            return;
        };
        let hold = &mut self.holds[at].hold;
        if frame <= hold.from {
            self.holds.remove(at);
            return;
        }
        hold.until = Some(frame);
    }

    pub fn retime(&mut self, id: HoldId, from: u32, until: Option<u32>) {
        let Some(at) = self.at(id) else {
            return;
        };
        let (side, start) = (self.holds[at].hold.hand(), self.holds[at].hold.from);
        let others = || {
            self.iter()
                .enumerate()
                .filter(move |(index, other)| {
                    *index != at && side.is_some() && other.hand() == side
                })
                .map(|(_, other)| other)
        };
        let previous = others()
            .filter(|other| other.from < start)
            .filter_map(|other| other.until)
            .max()
            .unwrap_or(0);
        let next = others()
            .map(|other| other.from)
            .filter(|other| *other > start)
            .min();
        let from = from.max(previous);
        let until = match (until, next) {
            (Some(until), Some(next)) => Some(until.min(next)),
            (None, Some(next)) => Some(next),
            (until, None) => until,
        };
        if until.is_some_and(|until| until <= from) {
            return;
        }
        let mut moved = self.holds.remove(at);
        moved.hold.from = from;
        moved.hold.until = until;
        self.insert(moved);
    }

    pub fn set_reach(&mut self, id: HoldId, reach: f32) {
        if let Some(hold) = self.get_mut(id)
            && reach.is_finite()
            && reach > 0.0
        {
            hold.target.reach = reach;
        }
    }

    pub fn set_ease(&mut self, id: HoldId, ease: u32) {
        if let Some(hold) = self.get_mut(id) {
            hold.ease = ease.min(MAX_EASE);
        }
    }

    pub fn remove(&mut self, id: HoldId) -> Option<Hold> {
        let at = self.at(id)?;
        Some(self.holds.remove(at).hold)
    }

    #[must_use]
    pub fn lanes(&self) -> Vec<LaneHolds> {
        let mut lanes: Vec<LaneHolds> = Side::BOTH
            .into_iter()
            .map(|side| LaneHolds {
                lane: HoldLane::Hand(side),
                holds: self.ids(|hold| hold.hand() == Some(side)),
            })
            .filter(|lane| !lane.holds.is_empty())
            .collect();
        let mut pins: Vec<Vec<&Held>> = Vec::new();
        for held in self
            .holds
            .iter()
            .filter(|held| matches!(held.hold.holder, Holder::Pin { .. }))
        {
            match pins
                .iter_mut()
                .find(|lane| lane.iter().all(|other| !other.hold.overlaps(&held.hold)))
            {
                Some(lane) => lane.push(held),
                None => pins.push(vec![held]),
            }
        }
        lanes.extend(pins.into_iter().enumerate().map(|(at, holds)| LaneHolds {
            lane: HoldLane::Pin(at),
            holds: holds.into_iter().map(|held| held.id).collect(),
        }));
        lanes
    }

    pub(crate) fn from_list(holds: Vec<Hold>) -> Self {
        let mut target = Self::default();
        for hold in holds {
            target.lay(hold);
        }
        target
    }

    fn at(&self, id: HoldId) -> Option<usize> {
        self.holds.iter().position(|held| held.id == id)
    }

    fn get_mut(&mut self, id: HoldId) -> Option<&mut Hold> {
        let at = self.at(id)?;
        Some(&mut self.holds[at].hold)
    }

    fn insert(&mut self, held: Held) {
        let at = self
            .holds
            .partition_point(|other| other.hold.from <= held.hold.from);
        self.holds.insert(at, held);
    }

    fn ids(&self, desired: impl Fn(&Hold) -> bool) -> Vec<HoldId> {
        self.holds
            .iter()
            .filter(|held| desired(&held.hold))
            .map(|held| held.id)
            .collect()
    }
}
