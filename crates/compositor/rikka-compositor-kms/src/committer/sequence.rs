use std::time::Duration;

use rikka_compositor_reactor::Timestamp;

use crate::timing::nanos;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Sequence {
    count: u64,
    last: Option<(u32, Timestamp)>,
}

impl Sequence {
    pub(crate) fn extend(&mut self, counter: u32, at: Timestamp, refresh: Duration) -> u64 {
        self.count = match self.last {
            None => u64::from(counter),
            Some((last, _)) if counter != last => {
                self.count + u64::from(counter.wrapping_sub(last))
            }
            Some((_, before)) => self.count + refreshes_between(before, at, refresh).max(1),
        };
        self.last = Some((counter, at));
        self.count
    }
}

fn refreshes_between(before: Timestamp, at: Timestamp, refresh: Duration) -> u64 {
    let period = nanos(refresh).max(1);
    let elapsed = nanos(at.saturating_since(before));
    elapsed.saturating_add(period / 2) / period
}
