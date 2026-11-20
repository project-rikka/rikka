use std::time::Instant;

use rikka_animation::Presence;
use rikka_types::Size;

use crate::motion::track::Track;
use crate::motion::{Clock, Pose, Transition};

#[derive(Clone, Copy, Debug)]
struct Leg {
    inward: bool,
    from: Option<Pose>,
    start: f32,
}

impl Leg {
    const ARRIVING: Leg = Leg {
        inward: true,
        from: None,
        start: 0.0,
    };

    const DEPARTING: Leg = Leg {
        inward: false,
        from: None,
        start: 1.0,
    };

    fn pose(&self, transition: &Transition, progress: f32, size: Size) -> Pose {
        let (from, to, end) = match self.inward {
            true => (self.from.unwrap_or(transition.arrive), Pose::REST, 1.0),
            false => (self.from.unwrap_or(Pose::REST), transition.depart, 0.0),
        };
        let span = end - self.start;
        let amount = match span.abs() > f32::EPSILON {
            true => (progress - self.start) / span,
            false => 1.0,
        };
        from.blend(to, amount, size)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Step {
    pub(crate) pose: Pose,
    pub(crate) shown: f32,
    pub(crate) moving: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct Passage {
    presence: Track<Presence>,
    leg: Leg,
    born: Option<Instant>,
}

impl Passage {
    pub(crate) fn arriving() -> Passage {
        Passage {
            presence: Track::new(Presence::HIDDEN, None),
            leg: Leg::ARRIVING,
            born: None,
        }
    }

    pub(crate) fn departing() -> Passage {
        Passage {
            presence: Track::new(Presence::SHOWN, None),
            leg: Leg::DEPARTING,
            born: None,
        }
    }

    pub(crate) fn advance(
        &mut self,
        transition: &Transition,
        inward: bool,
        size: Size,
        clock: Clock,
        debut: Instant,
    ) -> Step {
        let progress = self.presence.course.progress();
        if inward != self.leg.inward {
            self.leg = Leg {
                inward,
                from: Some(self.leg.pose(transition, progress, size)),
                start: progress,
            };
        }
        if inward
            && progress == 0.0
            && self.presence.course.is_resting()
            && !self.waited(transition, clock.now, debut)
        {
            return Step {
                pose: self.leg.pose(transition, progress, size),
                shown: progress,
                moving: true,
            };
        }
        let curve = match inward {
            true => transition.enter,
            false => transition.exit,
        };
        let elapsed = self.presence.elapsed(clock);
        let presence = &mut self.presence.course;
        presence.advance(inward, curve, elapsed);
        let progress = presence.progress();
        Step {
            pose: self.leg.pose(transition, progress, size),
            shown: progress,
            moving: !presence.is_resting(),
        }
    }

    fn waited(&mut self, transition: &Transition, now: Instant, debut: Instant) -> bool {
        if now <= debut {
            return false;
        }
        let born = *self.born.get_or_insert(now);
        transition
            .delay
            .is_none_or(|delay| now.saturating_duration_since(born) >= delay.duration())
    }
}
