use std::time::{Duration, Instant};

use tokio::time::{Interval, MissedTickBehavior, interval_at};

const RESPONSE_SHARE: u32 = 3;

pub(crate) fn response_window(every: Duration) -> Duration {
    every / RESPONSE_SHARE
}

#[derive(Debug)]
pub(crate) struct Heartbeat {
    interval: Interval,
    timeout: Duration,
    waiting_since: Option<Instant>,
}

impl Heartbeat {
    pub(crate) fn new(every: Duration, timeout: Duration) -> Heartbeat {
        let start = tokio::time::Instant::now() + every;
        let mut interval = interval_at(start, every);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        Heartbeat {
            interval,
            timeout,
            waiting_since: None,
        }
    }

    pub(crate) async fn tick(&mut self) {
        self.interval.tick().await;
    }

    pub(crate) fn is_stale(&self) -> bool {
        self.waiting_since
            .is_some_and(|sent| sent.elapsed() >= self.timeout)
    }

    pub(crate) fn sent(&mut self) {
        self.waiting_since = Some(Instant::now());
    }

    pub(crate) fn record_pong(&mut self) {
        self.waiting_since = None;
    }
}
