use std::time::Duration;

use jiff::Timestamp;
use rikka_app::prelude::*;

const TICKING: &str = "date-time-ticking";
const SECOND: Duration = Duration::from_secs(1);

#[derive(State, Debug)]
pub(crate) struct Ticking {
    now: Tracked<Timestamp>,
}

impl Ticking {
    pub(crate) fn new() -> Self {
        Self {
            now: Tracked::new(Timestamp::now()),
        }
    }

    pub(crate) fn now(&self) -> Timestamp {
        *self.now
    }

    pub(crate) fn start(&mut self) {
        self.tick();
    }

    pub(crate) fn stop(&mut self) {
        cancel(TICKING);
    }

    fn tick(&mut self) {
        let now = Timestamp::now();
        self.now.set(now);
        let into = u64::try_from(now.subsec_nanosecond()).unwrap_or(0);
        let left = SECOND.saturating_sub(Duration::from_nanos(into));
        after_keyed(TICKING, left, |ticking: &mut Ticking| ticking.tick());
    }
}
