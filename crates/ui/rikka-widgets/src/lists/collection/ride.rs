use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use rikka_animation::Channel;
use rikka_ui::motion;
use rustc_hash::FxHashMap;

use crate::lists::collection::cells::Cells;
use crate::lists::collection::spans::Spans;

const SETTLED: f32 = 0.25;

const STILL: f32 = 8.0;

const LONGEST: Duration = Duration::from_secs(2);

pub(crate) fn rests(offset: f32, velocity: f32) -> bool {
    offset.abs() < SETTLED && velocity.abs() < STILL
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DriftState {
    pub(crate) drift: f32,
    pub(crate) from: f32,
}

impl DriftState {
    pub(crate) fn reveals(&self, top: f32, extent: f32) -> Option<(f32, f32)> {
        let head = top + self.drift.max(self.from);
        let foot = top + self.drift + extent;
        (foot > head).then_some((head, foot))
    }
}

#[derive(Clone, Default)]
pub(crate) struct Riders(Rc<RefCell<FxHashMap<u64, DriftState>>>);

impl Riders {
    pub(crate) fn shown(&self, id: u64) -> Option<DriftState> {
        self.0.borrow().get(&id).copied()
    }

    pub(crate) fn clear(&self) {
        self.0.borrow_mut().clear();
    }

    pub(crate) fn is(&self, other: &Riders) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    pub(crate) fn ride(&self, ride: &mut Ride, now: Instant, spans: &Spans) -> bool {
        let mut shown = self.0.borrow_mut();
        shown.clear();
        ride.frame(now, spans, &mut shown)
    }
}

impl std::fmt::Debug for Riders {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Riders")
            .field(&self.0.borrow().len())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Order {
    slot: usize,
    step: usize,
}

impl Order {
    pub(crate) fn row(band: usize) -> Order {
        Order {
            slot: band + 1,
            step: 0,
        }
    }

    pub(crate) fn leaving(before: usize, step: usize) -> Order {
        Order {
            slot: before,
            step: step + 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Rest {
    Band(usize),
    Below {
        anchor: Option<usize>,
        by: f32,
        extent: f32,
    },
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Leg {
    pub(crate) rest: Rest,
    pub(crate) order: Order,
    pub(crate) settle: f32,
    pub(crate) amplitude: f32,
    pub(crate) velocity: f32,
}

impl Leg {
    pub(crate) fn band(band: usize, amplitude: f32, velocity: f32) -> Leg {
        Leg::new(Rest::Band(band), Order::row(band), amplitude, velocity)
    }

    pub(crate) fn new(rest: Rest, order: Order, amplitude: f32, velocity: f32) -> Leg {
        Leg {
            rest,
            order,
            settle: 0.0,
            amplitude,
            velocity,
        }
    }

    fn swing(&self, elapsed: Duration) -> (f32, f32) {
        let mut channel = Channel::new(self.amplitude);
        channel.velocity = self.velocity;
        channel.advance(motion::GLIDE, 0.0, elapsed);
        (channel.value, channel.velocity)
    }

    pub(crate) fn top(&self, spans: &Spans) -> f32 {
        match self.rest {
            Rest::Band(band) => spans.top(band),
            Rest::Below { anchor, by, .. } => anchor.map_or(0.0, |band| spans.top(band + 1)) + by,
        }
    }

    pub(crate) fn extent(&self, spans: &Spans) -> f32 {
        match self.rest {
            Rest::Band(band) => spans.height(band),
            Rest::Below { extent, .. } => extent,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Ghost {
    pub(crate) id: u64,
    pub(crate) name: u64,
    pub(crate) index: usize,
    pub(crate) cells: Option<Cells>,
}

#[derive(Debug, Default)]
pub(crate) struct Ride {
    started: Option<Instant>,
    legs: FxHashMap<u64, Leg>,
    sequence: Vec<u64>,
    pub(crate) ghosts: Vec<Ghost>,
}

impl Ride {
    pub(crate) fn new(
        legs: FxHashMap<u64, Leg>,
        ghosts: Vec<Ghost>,
        started: Option<Instant>,
    ) -> Ride {
        let mut sequence: Vec<u64> = legs.keys().copied().collect();
        sequence.sort_unstable_by_key(|id| (legs[id].order, *id));
        Ride {
            started,
            legs,
            sequence,
            ghosts,
        }
    }

    fn elapsed(&self, at: Option<Instant>) -> Duration {
        match (self.started, at) {
            (Some(started), Some(at)) => at.saturating_duration_since(started),
            _ => Duration::ZERO,
        }
    }

    pub(crate) fn seat(&self, ghost: &Ghost, spans: &Spans) -> Option<(f32, f32)> {
        let leg = self.legs.get(&ghost.id)?;
        Some((leg.top(spans), leg.extent(spans)))
    }

    pub(crate) fn shown_at(
        &self,
        at: Option<Instant>,
        spans: &Spans,
    ) -> Vec<(u64, Leg, DriftState, f32)> {
        let mut shown = Vec::with_capacity(self.sequence.len());
        self.show(self.elapsed(at), spans, |id, leg, visible, velocity| {
            shown.push((id, *leg, visible, velocity));
        });
        shown
    }

    fn frame(
        &mut self,
        now: Instant,
        spans: &Spans,
        shown: &mut FxHashMap<u64, DriftState>,
    ) -> bool {
        self.started.get_or_insert(now);
        let elapsed = self.elapsed(Some(now));
        let resting = self.show(elapsed, spans, |id, _, visible, _| {
            shown.insert(id, visible);
        });
        if resting || elapsed >= LONGEST {
            shown.clear();
            return false;
        }
        true
    }

    fn show(
        &self,
        elapsed: Duration,
        spans: &Spans,
        mut visible: impl FnMut(u64, &Leg, DriftState, f32),
    ) -> bool {
        let mut resting = true;
        let mut covered = 0.0_f32;
        for id in &self.sequence {
            let leg = &self.legs[id];
            let (swing, velocity) = leg.swing(elapsed);
            resting &= rests(swing, velocity);
            let top = leg.top(spans);
            let drift = leg.settle + swing;
            visible(
                *id,
                leg,
                DriftState {
                    drift,
                    from: covered - top,
                },
                velocity,
            );
            covered = covered.max(top + drift + leg.extent(spans));
        }
        resting
    }
}
