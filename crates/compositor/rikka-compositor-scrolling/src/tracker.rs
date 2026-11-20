use std::collections::VecDeque;
use std::time::Duration;

const HISTORY: Duration = Duration::from_millis(150);
const DECELERATION: f64 = 0.997;

#[derive(Debug, Clone, Default)]
pub(crate) struct Tracker {
    history: VecDeque<(Duration, f64)>,
    travel: f64,
}

impl Tracker {
    pub(crate) fn push(&mut self, delta: f64, time: Duration) {
        if self.history.back().is_some_and(|(last, _)| time < *last) {
            return;
        }
        self.history.push_back((time, delta));
        self.travel += delta;
        while self
            .history
            .front()
            .is_some_and(|(first, _)| *first + HISTORY < time)
        {
            self.history.pop_front();
        }
    }

    pub(crate) fn travel(&self) -> f64 {
        self.travel
    }

    pub(crate) fn velocity(&self) -> f64 {
        let (Some((first, _)), Some((last, _))) = (self.history.front(), self.history.back())
        else {
            return 0.0;
        };
        let elapsed = (*last - *first).as_secs_f64();
        if elapsed == 0.0 {
            return 0.0;
        }
        self.history.iter().map(|(_, delta)| delta).sum::<f64>() / elapsed
    }

    pub(crate) fn projected(&self) -> f64 {
        self.travel - self.velocity() / (1000.0 * DECELERATION.ln())
    }
}
