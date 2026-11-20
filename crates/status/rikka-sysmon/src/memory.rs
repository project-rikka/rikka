use crate::reading;

#[derive(Clone, Copy, Debug, Default)]
pub struct MemorySample {
    pub total: u64,
    pub available: u64,
    pub used: u64,
    pub cached: u64,
    pub committed: u64,
    pub commit_limit: u64,
    pub paged_pool: u64,
    pub nonpaged_pool: u64,
    pub swap_total: u64,
    pub swap_used: u64,
}

impl MemorySample {
    #[must_use]
    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        (self.used as f32 / self.total as f32).clamp(0.0, 1.0)
    }
}

pub(crate) fn sample() -> MemorySample {
    let meminfo = reading::text("/proc/meminfo").unwrap_or_default();
    let read = |key: &str| {
        reading::field(&meminfo, key)
            .and_then(reading::leading_number)
            .unwrap_or_default()
            * 1024
    };
    let total = read("MemTotal");
    let available = read("MemAvailable").min(total);
    let swap_total = read("SwapTotal");
    MemorySample {
        total,
        available,
        used: total.saturating_sub(available),
        cached: read("Cached") + read("Buffers") + read("SReclaimable"),
        committed: read("Committed_AS"),
        commit_limit: read("CommitLimit"),
        paged_pool: read("SReclaimable"),
        nonpaged_pool: read("SUnreclaim"),
        swap_total,
        swap_used: swap_total.saturating_sub(read("SwapFree")),
    }
}
