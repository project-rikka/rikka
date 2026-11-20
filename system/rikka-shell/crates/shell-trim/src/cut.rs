use std::ops::Range;
use std::time::Duration;

use rikka_video::Keyframes;

const LEAST: Duration = Duration::from_millis(200);

const SLACK: Duration = Duration::from_millis(150);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Cut {
    whole: Duration,
    start: Duration,
    end: Duration,
}

impl Cut {
    #[must_use]
    pub(crate) fn whole(whole: Duration) -> Cut {
        Cut {
            whole,
            start: Duration::ZERO,
            end: whole,
        }
    }

    #[must_use]
    pub(crate) fn length(self) -> Duration {
        self.whole
    }

    #[must_use]
    pub(crate) fn start(self) -> Duration {
        self.start
    }

    #[must_use]
    pub(crate) fn end(self) -> Duration {
        self.end
    }

    #[must_use]
    pub(crate) fn kept_duration(self) -> Duration {
        self.end.saturating_sub(self.start)
    }

    #[must_use]
    pub(super) fn is_whole(self) -> bool {
        self.start == Duration::ZERO && self.end == self.whole
    }

    #[must_use]
    pub(crate) fn holds(self, at: Duration) -> bool {
        at >= self.start && at + SLACK < self.end
    }

    #[must_use]
    pub(crate) fn span(self) -> Range<Duration> {
        self.start..self.end
    }

    pub(super) fn started_at(&mut self, at: Duration) -> bool {
        let last = self.end.saturating_sub(LEAST);
        self.moved(at.min(last), self.end)
    }

    pub(super) fn ended_at(&mut self, at: Duration) -> bool {
        let least = (self.start + LEAST).min(self.whole);
        self.moved(self.start, at.clamp(least, self.whole))
    }

    pub(crate) fn settled(&mut self, keyframes: &Keyframes) -> bool {
        let last = self.end.saturating_sub(LEAST);
        let start = keyframes
            .nearest(self.start)
            .filter(|start| *start <= last)
            .or_else(|| keyframes.at_or_before(last))
            .unwrap_or(Duration::ZERO);
        self.moved(start, self.end)
    }

    fn moved(&mut self, start: Duration, end: Duration) -> bool {
        let moved = Cut {
            whole: self.whole,
            start,
            end,
        };
        let changed = moved != *self;
        *self = moved;
        changed
    }
}
