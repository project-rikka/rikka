use std::time::Duration;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Progress {
    pub transferred: u64,
    pub total: Option<u64>,
    pub bytes_per_second: f64,
    pub elapsed: Duration,
}

impl Progress {
    pub(crate) fn finished(transferred: u64, total: Option<u64>, elapsed: Duration) -> Progress {
        let seconds = elapsed.as_secs_f64();

        #[expect(
            clippy::cast_precision_loss,
            reason = "a transfer rate needs no more precision"
        )]
        let bytes = transferred as f64;

        Progress {
            transferred,
            total,
            bytes_per_second: if seconds > 0.0 { bytes / seconds } else { 0.0 },
            elapsed,
        }
    }

    #[must_use]
    pub fn fraction(&self) -> Option<f32> {
        let total = self.total?;
        if total == 0 {
            return Some(1.0);
        }

        #[expect(
            clippy::cast_precision_loss,
            reason = "a progress bar needs no more precision"
        )]
        let fraction = self.transferred as f32 / total as f32;
        Some(fraction.clamp(0.0, 1.0))
    }

    #[must_use]
    pub fn remaining(&self) -> Option<u64> {
        self.total
            .map(|total| total.saturating_sub(self.transferred))
    }

    #[must_use]
    pub fn eta(&self) -> Option<Duration> {
        let remaining = self.remaining()?;
        if remaining == 0 {
            return Some(Duration::ZERO);
        }
        if self.bytes_per_second < 1.0 {
            return None;
        }

        #[expect(
            clippy::cast_precision_loss,
            reason = "an estimate to the second needs no more precision"
        )]
        let seconds = remaining as f64 / self.bytes_per_second;
        Some(Duration::try_from_secs_f64(seconds).unwrap_or(Duration::MAX))
    }

    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.total.is_some_and(|total| self.transferred >= total)
    }
}

pub type ProgressCallback = Box<dyn FnMut(Progress) -> crate::Flow + Send + 'static>;
