use std::ops::Range;
use std::time::Duration;

use rikka_model::mmd::FRAMES_PER_SECOND;

use crate::encoding::FrameRate;
use crate::export::Export;

const TIMELINE_RATE: u64 = FRAMES_PER_SECOND as u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    first: u32,
    last: u32,
    rate: FrameRate,
}

impl Plan {
    pub fn new(first: u32, last: u32, rate: FrameRate) -> Self {
        Self {
            first,
            last: last.max(first),
            rate,
        }
    }

    pub fn for_export(export: &Export) -> Self {
        Self::new(export.first, export.last, export.encoding.frame_rate)
    }

    fn numerator(&self) -> u64 {
        u64::from(self.rate.numerator())
    }

    fn denominator(&self) -> u64 {
        u64::from(self.rate.denominator())
    }

    pub fn frames(&self) -> u64 {
        let spanned = u64::from(self.last - self.first + 1);
        let scale = TIMELINE_RATE * self.denominator();
        ((spanned * self.numerator() + scale / 2) / scale).max(1)
    }

    pub fn time(&self, index: u64) -> f32 {
        let start = f64::from(self.first) / TIMELINE_RATE as f64;
        let offset = index as f64 * self.denominator() as f64 / self.numerator() as f64;
        (start + offset) as f32
    }

    pub fn duration(&self) -> Duration {
        Duration::from_nanos(
            (u128::from(self.frames()) * u128::from(self.denominator()) * 1_000_000_000
                / u128::from(self.numerator())) as u64,
        )
    }

    pub fn samples(&self, index: u64, sample_rate: u32) -> Range<u64> {
        let rate = u64::from(sample_rate);
        let start = u64::from(self.first) * rate / TIMELINE_RATE;
        let at = |index: u64| {
            start
                + (u128::from(index) * u128::from(rate) * u128::from(self.denominator())
                    / u128::from(self.numerator())) as u64
        };
        at(index)..at(index + 1)
    }
}
