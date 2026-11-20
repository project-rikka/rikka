use std::ops::Range;
use std::time::Instant;

use rikka_ui::hash_of;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::lists::collection::cells::Cells;
use crate::lists::collection::ride::{DriftState, Ghost, Leg, Order, Rest, Ride, rests};
use crate::lists::collection::spans::Spans;
use crate::lists::collection::{AnchorPoint, CollectionState, NUDGE};

const LEFT: &str = "left";

#[derive(Clone, Copy, Debug)]
struct Pair {
    was: usize,
    now: usize,
    steady: bool,
}

#[derive(Clone, Copy, Debug)]
enum Anchor {
    Top,
    Row(Pair),
    Loose,
}

#[derive(Clone, Copy, Debug)]
struct Carry {
    shown: DriftState,
    velocity: f32,
}

impl Carry {
    const STILL: Carry = Carry {
        shown: DriftState {
            drift: 0.0,
            from: f32::NEG_INFINITY,
        },
        velocity: 0.0,
    };
}

#[derive(Clone, Debug)]
struct MovingRow {
    ghost: Ghost,
    order: Order,
    visible_extent: f32,
    extent: f32,
    shown: Option<(f32, f32)>,
    velocity: f32,
}

#[derive(Debug)]
pub(crate) struct Shuffle {
    pairs: Vec<Pair>,
    window: Range<usize>,
    count: usize,
    tops: Vec<f32>,
    names: Vec<u64>,
    offset: f32,
    anchor: Anchor,
    carry: FxHashMap<u64, Carry>,
    moving: Vec<MovingRow>,
    previous: Option<Cells>,
    generation: u64,
    since: Option<Instant>,
}

pub(crate) struct Stage<'a> {
    pub(crate) spans: &'a Spans,
    pub(crate) offset: f32,
    pub(crate) room: f32,
    pub(crate) window: Range<usize>,
}

struct ExitingRow {
    ghost: Ghost,
    visible_extent: f32,
    extent: f32,
    head: f32,
    foot: f32,
    velocity: f32,
}

struct Gap {
    above: Option<usize>,
    inserted: Range<usize>,
    leaving: Vec<ExitingRow>,
    floor: Option<f32>,
    anchor_offset: f32,
    came: f32,
}

struct Deltas {
    rows: Vec<f32>,
    top: f32,
    bottom: f32,
}

impl Deltas {
    fn before(&self, gap: usize) -> f32 {
        match gap.checked_sub(1) {
            Some(row) => self.rows[row],
            None => self.top,
        }
    }
}

fn visible(from: f32, to: f32, room: f32) -> f32 {
    (to.min(room) - from.max(0.0)).max(0.0)
}

fn visible_extent(at: f32, extent: f32, room: f32) -> bool {
    at + extent > NUDGE && at < room - NUDGE
}

fn matched(
    window: &Range<usize>,
    names: &[u64],
    before: usize,
    after: usize,
    name: &dyn Fn(usize) -> u64,
) -> Vec<Pair> {
    let mut seats: FxHashMap<u64, usize> = FxHashMap::default();
    for (band, was) in window.clone().zip(names) {
        seats.entry(*was).or_insert(band);
    }
    let reach = window.len();
    let from = window
        .start
        .saturating_sub(reach + before.saturating_sub(after));
    let to = (window.end + reach + after.saturating_sub(before)).min(after);
    let mut pairs = Vec::with_capacity(seats.len());
    for now in from..to {
        if seats.is_empty() {
            break;
        }
        if let Some(was) = seats.remove(&name(now)) {
            pairs.push(Pair {
                was,
                now,
                steady: false,
            });
        }
    }
    pairs.sort_unstable_by_key(|pair| pair.was);
    steady(&mut pairs);
    pairs
}

fn steady(pairs: &mut [Pair]) {
    let mut tails: Vec<usize> = Vec::new();
    let mut before: Vec<Option<usize>> = vec![None; pairs.len()];
    for (at, pair) in pairs.iter().enumerate() {
        let rank = tails.partition_point(|tail| pairs[*tail].now < pair.now);
        before[at] = rank.checked_sub(1).map(|rank| tails[rank]);
        match tails.get_mut(rank) {
            Some(tail) => *tail = at,
            None => tails.push(at),
        }
    }
    let mut walk = tails.last().copied();
    while let Some(at) = walk {
        pairs[at].steady = true;
        walk = before[at];
    }
}

impl CollectionState {
    pub(crate) fn reshuffle(
        &mut self,
        bands: usize,
        name: &dyn Fn(usize) -> u64,
        follow: bool,
    ) -> Option<Shuffle> {
        let window = self.window.clone();
        let count = self.spans.count();
        let ride = self.ride.take();
        if window.is_empty() || window.end > count || self.names.len() != window.len() {
            return None;
        }
        let offset = self.viewing.scroll.rendered();
        let tops: Vec<f32> = (window.start..=window.end)
            .map(|band| self.spans.top(band))
            .collect();
        let mut shuffle = Shuffle {
            pairs: Vec::new(),
            window: window.clone(),
            count,
            tops,
            names: self.names.clone(),
            offset,
            anchor: Anchor::Loose,
            carry: FxHashMap::default(),
            moving: Vec::new(),
            previous: self.previous.clone(),
            generation: 0,
            since: None,
        };
        if let Some(ride) = ride {
            shuffle.carry_from(ride, &self.spans, self.frame);
        }

        shuffle.pairs = matched(&window, &self.names, count, bands, name);
        self.remap(&shuffle.pairs, &window, count, bands);
        shuffle.anchor = self.anchor_shuffled(&shuffle, follow);
        let first = shuffle.pairs.iter().find(|pair| pair.steady);
        let last = shuffle.pairs.iter().rev().find(|pair| pair.steady);
        self.window = match (first, last) {
            (Some(first), Some(last)) => first.now..last.now + 1,
            _ => 0..0,
        };
        self.generation += 1;
        shuffle.generation = self.generation;
        Some(shuffle)
    }

    fn remap(&mut self, pairs: &[Pair], window: &Range<usize>, count: usize, bands: usize) {
        let by_now: FxHashMap<usize, usize> =
            pairs.iter().map(|pair| (pair.now, pair.was)).collect();
        let first = pairs.iter().find(|pair| pair.steady).copied();
        let last = pairs.iter().rev().find(|pair| pair.steady).copied();
        self.spans.remap(bands, |now| {
            if let Some(was) = by_now.get(&now) {
                return Some(*was);
            }
            let (first, last) = (first?, last?);
            if now < first.now {
                return (now + first.was)
                    .checked_sub(first.now)
                    .filter(|was| *was < window.start);
            }
            if now > last.now {
                return Some(now - last.now + last.was)
                    .filter(|was| *was >= window.end && *was < count);
            }
            None
        });
    }

    fn anchor_shuffled(&mut self, shuffle: &Shuffle, follow: bool) -> Anchor {
        let room = self.room;
        let offset = shuffle.offset;
        if room <= 0.0 || (follow && self.viewing.holds_end()) {
            return Anchor::Loose;
        }
        let shown = |pair: &&Pair| {
            let (at, extent) = shuffle.previous_span(pair.was);
            visible_extent(at, extent, room)
        };
        let mut steady = shuffle.pairs.iter().filter(|pair| pair.steady);
        let anchor = match (follow, offset <= NUDGE) {
            (false, true) => Anchor::Top,
            (true, _) => steady
                .rev()
                .find(shown)
                .map_or(Anchor::Loose, |pair| Anchor::Row(*pair)),
            (false, false) => steady
                .find(shown)
                .map_or(Anchor::Loose, |pair| Anchor::Row(*pair)),
        };
        let furthest = (self.spans.content() - room).max(0.0);
        let target = match anchor {
            Anchor::Top => 0.0,
            Anchor::Row(pair) => self.spans.top(pair.now) - shuffle.previous_span(pair.was).0,
            Anchor::Loose => offset,
        }
        .clamp(0.0, furthest);
        let shift = target - offset;
        if shift != 0.0 {
            self.viewing.scroll.shift(shift);
            self.origin += shift;
        }
        self.pending_anchor = match anchor {
            Anchor::Top => Some(AnchorPoint {
                band: 0,
                screen: -target,
            }),
            Anchor::Row(pair) => Some(AnchorPoint {
                band: pair.now,
                screen: self.spans.top(pair.now) - target,
            }),
            Anchor::Loose => None,
        };
        self.anchor_at(self.viewing.scroll.rendered());
        anchor
    }
}

impl Shuffle {
    fn carry_from(&mut self, mut ride: Ride, spans: &Spans, at: Option<Instant>) {
        self.since = at;
        let mut ghosts: FxHashMap<u64, Ghost> = std::mem::take(&mut ride.ghosts)
            .into_iter()
            .map(|ghost| (ghost.id, ghost))
            .collect();
        for (id, leg, shown, velocity) in ride.shown_at(at, spans) {
            match leg.rest {
                Rest::Band(_) => {
                    self.carry.insert(id, Carry { shown, velocity });
                }
                Rest::Below { .. } => {
                    let Some(ghost) = ghosts.remove(&id) else {
                        continue;
                    };
                    let top = leg.top(spans) - self.offset;
                    let extent = leg.extent(spans);
                    self.moving.push(MovingRow {
                        ghost: Ghost {
                            cells: ghost.cells.or_else(|| self.previous.clone()),
                            ..ghost
                        },
                        order: leg.order,
                        visible_extent: top + shown.drift,
                        extent,
                        shown: shown.reveals(top, extent),
                        velocity,
                    });
                }
            }
        }
        self.moving.sort_unstable_by_key(|moving| moving.order);
    }

    fn previous_span(&self, band: usize) -> (f32, f32) {
        let at = band - self.window.start;
        let top = self.tops[at];
        (top - self.offset, self.tops[at + 1] - top)
    }

    pub(crate) fn ride(&self, stage: &Stage<'_>, name: &dyn Fn(usize) -> u64) -> Option<Ride> {
        let steady: Vec<Pair> = self
            .pairs
            .iter()
            .copied()
            .filter(|pair| pair.steady)
            .collect();
        if steady.is_empty() || stage.room <= 0.0 {
            return None;
        }
        let present: FxHashSet<u64> = stage.window.clone().map(name).collect();
        let (returning, departing): (Vec<&MovingRow>, Vec<&MovingRow>) = self
            .moving
            .iter()
            .partition(|moving| present.contains(&moving.ghost.name));
        let mut plan = Plan {
            shuffle: self,
            stage,
            name,
            returning: returning
                .into_iter()
                .map(|moving| (moving.ghost.name, moving))
                .collect(),
            departing,
            legs: FxHashMap::default(),
            ghosts: Vec::new(),
        };
        let gaps: Vec<Gap> = (0..=steady.len()).map(|at| plan.gap(&steady, at)).collect();
        let deltas = plan.deltas(&steady, &gaps);
        plan.steady(&steady, &deltas);
        plan.entering(&steady, &gaps, &deltas);
        for (at, gap) in gaps.into_iter().enumerate() {
            plan.opened(gap, deltas.before(at));
        }
        plan.finished()
    }
}

struct Plan<'a, 'b> {
    shuffle: &'a Shuffle,
    stage: &'a Stage<'b>,
    name: &'a dyn Fn(usize) -> u64,
    returning: FxHashMap<u64, &'a MovingRow>,
    departing: Vec<&'a MovingRow>,
    legs: FxHashMap<u64, Leg>,
    ghosts: Vec<Ghost>,
}

impl Plan<'_, '_> {
    fn now_at(&self, band: usize) -> (f32, f32) {
        let spans = self.stage.spans;
        (spans.top(band) - self.stage.offset, spans.height(band))
    }

    fn before_named(&self, band: usize) -> u64 {
        self.shuffle.names[band - self.shuffle.window.start]
    }

    fn carried_of(&self, id: u64) -> Carry {
        self.shuffle.carry.get(&id).copied().unwrap_or(Carry::STILL)
    }

    fn leaving(
        &self,
        left: Range<usize>,
        after: Option<Order>,
        until: Option<Order>,
    ) -> Vec<ExitingRow> {
        let room = self.stage.room;
        let Some(previous) = &self.shuffle.previous else {
            return Vec::new();
        };
        let shows = |(head, foot): (f32, f32)| visible(head, foot, room) > NUDGE;
        let generation = self.shuffle.generation;
        let rows = left.filter_map(|band| {
            let named = self.before_named(band);
            let carry = self.carried_of(named);
            let (at, extent) = self.shuffle.previous_span(band);
            let (head, foot) = carry
                .shown
                .reveals(at, extent)
                .filter(|visible_extent| shows(*visible_extent))?;
            let leaving = ExitingRow {
                ghost: Ghost {
                    id: hash_of((LEFT, named, generation)),
                    name: named,
                    index: band,
                    cells: Some(previous.clone()),
                },
                visible_extent: at + carry.shown.drift,
                extent,
                head,
                foot,
                velocity: carry.velocity,
            };
            Some((Order::row(band), leaving))
        });
        let departing = &self.departing;
        let from = after.map_or(0, |after| {
            departing.partition_point(|moving| moving.order <= after)
        });
        let to = until.map_or(departing.len(), |until| {
            departing.partition_point(|moving| moving.order < until)
        });
        let ghosts = departing[from..to.max(from)].iter().filter_map(|moving| {
            let (head, foot) = moving
                .shown
                .filter(|visible_extent| shows(*visible_extent))?;
            let leaving = ExitingRow {
                ghost: moving.ghost.clone(),
                visible_extent: moving.visible_extent,
                extent: moving.extent,
                head,
                foot,
                velocity: moving.velocity,
            };
            Some((moving.order, leaving))
        });
        let mut leaving: Vec<(Order, ExitingRow)> = rows.chain(ghosts).collect();
        leaving.sort_unstable_by_key(|(order, _)| *order);
        leaving.into_iter().map(|(_, leaving)| leaving).collect()
    }

    fn gap(&self, steady: &[Pair], at: usize) -> Gap {
        let window = &self.shuffle.window;
        let bands = self.stage.spans.count();
        let room = self.stage.room;
        let above = at.checked_sub(1).map(|at| steady[at]);
        let below = steady.get(at).copied();
        let left = above.map_or(window.start, |pair| pair.was + 1)
            ..below.map_or(window.end, |pair| pair.was);
        let inserted = match (above, below) {
            (Some(above), Some(below)) => above.now + 1..below.now,
            (None, Some(below)) => {
                let from = match window.start {
                    0 => 0,
                    start => (below.now + start).saturating_sub(below.was).min(below.now),
                };
                from..below.now
            }
            (Some(above), None) => {
                let to = match window.end >= self.shuffle.count {
                    true => bands,
                    false => (above.now + 1 + left.len()).min(bands),
                };
                above.now + 1..to.max(above.now + 1)
            }
            (None, None) => 0..0,
        };
        let leaving = self.leaving(
            left,
            above.map(|pair| Order::row(pair.was)),
            below.map(|pair| Order::row(pair.was)),
        );
        let floor = leaving.iter().map(|leaving| leaving.foot).reduce(f32::max);
        let anchor_offset = match (leaving.first(), floor) {
            (Some(first), Some(floor)) => visible(first.head, floor, room),
            _ => 0.0,
        };
        let came = match inserted.is_empty() {
            true => 0.0,
            false => visible(
                self.now_at(inserted.start).0,
                self.stage.spans.top(inserted.end) - self.stage.offset,
                room,
            ),
        };
        Gap {
            above: above.map(|pair| pair.now).or(inserted.start.checked_sub(1)),
            inserted,
            leaving,
            floor,
            anchor_offset,
            came,
        }
    }

    fn deltas(&self, steady: &[Pair], gaps: &[Gap]) -> Deltas {
        let room = self.stage.room;
        let moved = |pair: Pair| self.shuffle.previous_span(pair.was).0 - self.now_at(pair.now).0;
        let (anchor, anchored) = match self.shuffle.anchor {
            Anchor::Top => (
                0,
                self.stage.offset - self.shuffle.offset + gaps[0].anchor_offset - gaps[0].came,
            ),
            Anchor::Row(pair) => (
                steady
                    .iter()
                    .position(|steady| steady.was == pair.was)
                    .unwrap_or(0),
                moved(pair),
            ),
            Anchor::Loose => (0, moved(steady[0])),
        };
        let mut rows = vec![0.0; steady.len()];
        rows[anchor] = anchored.clamp(-room, room);
        for at in anchor + 1..rows.len() {
            rows[at] = rows[at - 1] + gaps[at].anchor_offset - gaps[at].came;
        }
        for at in (0..anchor).rev() {
            rows[at] = rows[at + 1] - gaps[at + 1].anchor_offset + gaps[at + 1].came;
        }
        let last = rows.len() - 1;
        Deltas {
            top: rows[0] - gaps[0].anchor_offset + gaps[0].came,
            bottom: rows[last] + gaps[last + 1].anchor_offset - gaps[last + 1].came,
            rows,
        }
    }

    fn steady(&mut self, steady: &[Pair], deltas: &Deltas) {
        let room = self.stage.room;
        for (at, pair) in steady.iter().enumerate() {
            let id = self.before_named(pair.was);
            let delta = deltas.rows[at];
            let carry = self.carried_of(id);
            let (drift, velocity) = (carry.shown.drift, carry.velocity);
            let (was, was_extent) = self.shuffle.previous_span(pair.was);
            let (now, extent) = self.now_at(pair.now);
            let was_seen = visible_extent(was + drift, was_extent, room);
            let start = match was_seen {
                true => was + drift,
                false => now + delta + drift,
            };
            let end = match visible_extent(now, extent, room) || !was_seen {
                true => now,
                false => was - delta,
            };
            if self.stage.window.contains(&pair.now) || !was_seen {
                let leg = Leg {
                    settle: end - now,
                    ..Leg::band(pair.now, start - end, velocity)
                };
                self.legs.insert(id, leg);
                continue;
            }
            let rest = Rest::Below {
                anchor: None,
                by: end + self.stage.offset,
                extent,
            };
            let leg = Leg::new(rest, Order::row(pair.now), start - end, velocity);
            self.legs.insert(id, leg);
            self.ghosts.push(Ghost {
                id,
                name: id,
                index: pair.now,
                cells: None,
            });
        }
    }

    fn entering(&mut self, steady: &[Pair], gaps: &[Gap], deltas: &Deltas) {
        let (first, last) = (steady[0].now, steady[steady.len() - 1].now);
        let (head, tail) = (&gaps[0].inserted, &gaps[gaps.len() - 1].inserted);
        for band in self.stage.window.clone() {
            if (first..=last).contains(&band) || head.contains(&band) || tail.contains(&band) {
                continue;
            }
            let delta = match band < first {
                true => deltas.top,
                false => deltas.bottom,
            };
            self.enter(band, delta);
        }
    }

    fn returned(&self, band: usize, id: u64) -> Option<Leg> {
        let moving = self.returning.get(&id)?;
        Some(Leg::band(
            band,
            moving.visible_extent - self.now_at(band).0,
            moving.velocity,
        ))
    }

    fn enter(&mut self, band: usize, delta: f32) {
        let id = (self.name)(band);
        let leg = self.returned(band, id).unwrap_or_else(|| {
            let carry = self.carried_of(id);
            Leg::band(band, delta + carry.shown.drift, carry.velocity)
        });
        self.legs.insert(id, leg);
    }

    fn opened(&mut self, gap: Gap, before: f32) {
        if gap.inserted.is_empty() && gap.leaving.is_empty() {
            return;
        }
        let above = gap.above.map(|band| (band, (self.name)(band)));
        if let Some((band, id)) = above
            && !self.legs.contains_key(&id)
        {
            self.enter(band, before);
        }
        let (lead, pace) = match above {
            Some((_, id)) => {
                let leg = &self.legs[&id];
                (leg.settle + leg.amplitude, leg.velocity)
            }
            None => (before, 0.0),
        };
        let rising = lead - gap.came;
        for band in gap.inserted.clone() {
            if self.stage.window.contains(&band) {
                let id = (self.name)(band);
                let leg = self
                    .returned(band, id)
                    .unwrap_or_else(|| Leg::band(band, rising, pace));
                self.legs.insert(id, leg);
            }
        }
        let Some(floor) = gap.floor else {
            return;
        };
        let floor = floor.min(self.stage.room);
        let ceiling = gap.above.map_or(0.0, |band| self.stage.spans.top(band + 1));
        let shift = ceiling - self.stage.offset + gap.came - floor;
        for (step, leaving) in gap.leaving.into_iter().enumerate() {
            let end = leaving.visible_extent + shift;
            let rest = Rest::Below {
                anchor: gap.above,
                by: end + self.stage.offset - ceiling,
                extent: leaving.extent,
            };
            let order = Order::leaving(gap.inserted.end, step);
            let leg = Leg::new(rest, order, leaving.visible_extent - end, leaving.velocity);
            self.legs.insert(leaving.ghost.id, leg);
            self.ghosts.push(leaving.ghost);
        }
    }

    fn finished(self) -> Option<Ride> {
        let still = self
            .legs
            .values()
            .all(|leg| rests(leg.amplitude, leg.velocity) && leg.settle.abs() < NUDGE);
        match still && self.ghosts.is_empty() {
            true => None,
            false => Some(Ride::new(self.legs, self.ghosts, self.shuffle.since)),
        }
    }
}
