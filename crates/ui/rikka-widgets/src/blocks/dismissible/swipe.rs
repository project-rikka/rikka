use std::time::Instant;

use rikka_types::Point;
use rikka_ui::DRAG_SLOP;

use super::velocity::Velocity;

const AWAY: f32 = 0.4;

const FLICK: f32 = 500.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Pointer,
    Touchpad,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Decision {
    Pending,
    Claimed,
    Refused,
}

#[derive(Debug)]
pub(super) struct Swipe {
    source: Source,
    origin: Point,
    reached: Point,
    along: f32,
    decision: Decision,
    velocity: Velocity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Step {
    Undecided,
    Claimed,
    Followed,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Release {
    pub(super) along: f32,
    pub(super) velocity: f32,
    pub(super) away: bool,
}

impl Swipe {
    #[must_use]
    pub(super) fn pressed(at: Point) -> Swipe {
        Swipe::from(Source::Pointer, at)
    }

    #[must_use]
    pub(super) fn stroking() -> Swipe {
        Swipe::from(Source::Touchpad, Point::ZERO)
    }

    fn from(source: Source, origin: Point) -> Swipe {
        Swipe {
            source,
            origin,
            reached: origin,
            along: 0.0,
            decision: Decision::Pending,
            velocity: Velocity::default(),
        }
    }

    #[must_use]
    pub(super) fn is_claimed(&self) -> bool {
        self.decision == Decision::Claimed
    }

    #[must_use]
    pub(super) fn is_touchpad(&self) -> bool {
        self.source == Source::Touchpad
    }

    #[must_use]
    pub(super) fn along(&self) -> f32 {
        self.along
    }

    pub(super) fn moved(&mut self, at: Point, now: Instant) -> Step {
        match self.source {
            Source::Pointer => self.advance(at, now),
            Source::Touchpad => Step::Undecided,
        }
    }

    pub(super) fn stroked(&mut self, delta: Point, now: Instant) -> Step {
        match self.source {
            Source::Touchpad => self.advance(self.reached + delta, now),
            Source::Pointer => Step::Undecided,
        }
    }

    #[must_use]
    pub(super) fn release(&self, now: Instant, width: f32) -> Release {
        let velocity = self.velocity.at(now);
        let away = velocity >= FLICK || (self.along >= width * AWAY && velocity > -FLICK);
        Release {
            along: self.along,
            velocity,
            away,
        }
    }

    fn advance(&mut self, at: Point, now: Instant) -> Step {
        self.reached = at;
        match self.decision {
            Decision::Refused => Step::Undecided,
            Decision::Claimed => {
                self.follow(now);
                Step::Followed
            }
            Decision::Pending if at.distance(self.origin) <= DRAG_SLOP => Step::Undecided,
            Decision::Pending => {
                let travel = at - self.origin;
                if travel.x <= travel.y.abs() {
                    self.decision = Decision::Refused;
                    return Step::Undecided;
                }
                self.decision = Decision::Claimed;
                self.origin.x += DRAG_SLOP;
                self.follow(now);
                Step::Claimed
            }
        }
    }

    fn follow(&mut self, now: Instant) {
        self.along = (self.reached.x - self.origin.x).max(0.0);
        self.velocity.add(now, self.along);
    }
}
