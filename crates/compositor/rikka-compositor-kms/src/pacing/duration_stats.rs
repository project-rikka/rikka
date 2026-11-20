use std::time::Duration;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DurationStats {
    pub(crate) count: u64,
    pub(crate) total: Duration,
    pub(crate) longest: Duration,
}

impl DurationStats {
    pub(crate) fn add(&mut self, measured: Duration) {
        self.count += 1;
        self.total += measured;
        self.longest = self.longest.max(measured);
    }

    pub(crate) fn mean(&self) -> Duration {
        u32::try_from(self.count)
            .ok()
            .and_then(|count| self.total.checked_div(count))
            .unwrap_or_default()
    }
}
