use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
pub(super) struct Flight {
    from: f32,
    speed: f32,
    start: Option<Instant>,
}

impl Flight {
    #[must_use]
    pub(super) fn new(from: f32, velocity: f32, away: f32, within: Duration) -> Flight {
        let least = (away - from).max(0.0) / within.as_secs_f32();
        Flight {
            from,
            speed: velocity.max(least),
            start: None,
        }
    }

    #[must_use]
    pub(super) fn at(&mut self, now: Instant, away: f32) -> f32 {
        let start = *self.start.get_or_insert(now);
        let flown = self.speed * now.saturating_duration_since(start).as_secs_f32();
        (self.from + flown).min(away)
    }
}
