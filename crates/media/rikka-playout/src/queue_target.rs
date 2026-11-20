use std::collections::VecDeque;
use std::time::{Duration, Instant};

const LOWEST: Duration = Duration::from_millis(30);
const HIGHEST: Duration = Duration::from_millis(180);
const MARGIN: Duration = Duration::from_millis(15);
const BURST_WINDOW: Duration = Duration::from_secs(3);
const UNDERRUN_STEP: Duration = Duration::from_millis(10);
const UNDERRUN_CEILING: Duration = Duration::from_millis(100);
const CALM: Duration = Duration::from_secs(20);
const EASE: Duration = Duration::from_millis(5);

#[derive(Debug)]
pub struct QueueTarget {
    last_queued: Option<Duration>,
    pushed: Duration,
    bursts: VecDeque<(Instant, Duration)>,
    underruns: Option<u32>,
    extra: Duration,
    calm_since: Option<Instant>,
}

impl Default for QueueTarget {
    fn default() -> QueueTarget {
        QueueTarget::new()
    }
}

impl QueueTarget {
    #[must_use]
    pub fn new() -> QueueTarget {
        QueueTarget {
            last_queued: None,
            pushed: Duration::ZERO,
            bursts: VecDeque::new(),
            underruns: None,
            extra: Duration::ZERO,
            calm_since: None,
        }
    }

    pub fn observe(&mut self, now: Instant, queued: Duration, underruns: u32) {
        if let Some(last) = self.last_queued {
            let consumed = (last + self.pushed).saturating_sub(queued);
            if !consumed.is_zero() {
                self.bursts.push_back((now, consumed));
            }
        }
        while self
            .bursts
            .front()
            .is_some_and(|(at, _)| now.saturating_duration_since(*at) > BURST_WINDOW)
        {
            self.bursts.pop_front();
        }
        self.last_queued = Some(queued);
        self.pushed = Duration::ZERO;
        let starved = self.underruns.is_some_and(|previous| underruns > previous);
        self.underruns = Some(underruns);
        if starved {
            self.extra = (self.extra + UNDERRUN_STEP).min(UNDERRUN_CEILING);
            self.calm_since = Some(now);
            return;
        }
        let calm = *self.calm_since.get_or_insert(now);
        if !self.extra.is_zero() && now.saturating_duration_since(calm) >= CALM {
            self.extra = self.extra.saturating_sub(EASE);
            self.calm_since = Some(now);
        }
    }

    pub fn wrote(&mut self, duration: Duration) {
        self.pushed += duration;
    }

    #[must_use]
    pub fn target(&self) -> Duration {
        let burst = self
            .bursts
            .iter()
            .map(|(_, consumed)| *consumed)
            .max()
            .unwrap_or_default();
        ((burst + MARGIN).max(LOWEST) + self.extra).min(HIGHEST)
    }

    pub fn reset(&mut self) {
        self.last_queued = None;
        self.pushed = Duration::ZERO;
        self.underruns = None;
    }
}
