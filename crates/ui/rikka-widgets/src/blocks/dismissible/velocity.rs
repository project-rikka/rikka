use std::collections::VecDeque;
use std::time::{Duration, Instant};

const WINDOW: Duration = Duration::from_millis(100);

const STILL: Duration = Duration::from_millis(50);

const SHORTEST_SPAN: Duration = Duration::from_millis(4);

#[derive(Debug, Default)]
pub(super) struct Velocity {
    samples: VecDeque<(Instant, f32)>,
}

impl Velocity {
    pub(super) fn add(&mut self, at: Instant, along: f32) {
        self.samples.push_back((at, along));
        while let Some(&(oldest, _)) = self.samples.front()
            && at.saturating_duration_since(oldest) > WINDOW
        {
            self.samples.pop_front();
        }
    }

    #[must_use]
    pub(super) fn at(&self, now: Instant) -> f32 {
        let (Some(&(first, from)), Some(&(last, to))) = (self.samples.front(), self.samples.back())
        else {
            return 0.0;
        };
        if now.saturating_duration_since(last) > STILL {
            return 0.0;
        }
        let span = last.saturating_duration_since(first);
        if span < SHORTEST_SPAN {
            return 0.0;
        }
        (to - from) / span.as_secs_f32()
    }
}
