use std::ops::Range;
use std::time::Duration;

use cut_project::Rate;

use crate::span::Span;

const NANOS: u128 = 1_000_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    span: Span,
    timeline: Rate,
    output: Rate,
}

impl Plan {
    #[must_use]
    pub fn new(span: Span, timeline: Rate, output: Rate) -> Plan {
        Plan {
            span,
            timeline,
            output,
        }
    }

    #[must_use]
    pub fn span(&self) -> Span {
        self.span
    }

    #[must_use]
    pub fn output(&self) -> Rate {
        self.output
    }

    #[must_use]
    pub fn frames(&self) -> u64 {
        let spanned = self.span.length() as u128
            * u128::from(self.output.numerator())
            * u128::from(self.timeline.denominator());
        let per = u128::from(self.output.denominator()) * u128::from(self.timeline.numerator());
        ((spanned + per / 2) / per).max(1) as u64
    }

    #[must_use]
    pub fn frame(&self, index: u64) -> i64 {
        let along = u128::from(self.output.denominator()) * u128::from(self.timeline.numerator());
        let per = u128::from(self.output.numerator()) * u128::from(self.timeline.denominator());
        let doubled = 2 * u128::from(index) * along;
        let nearest = match doubled > per {
            true => (doubled - per).div_ceil(2 * per),
            false => 0,
        };
        let offset = nearest.min(self.span.length() as u128 - 1) as i64;
        self.span.first() + offset
    }

    #[must_use]
    pub fn duration(&self) -> Duration {
        let nanos = u128::from(self.frames()) * u128::from(self.output.denominator()) * NANOS
            / u128::from(self.output.numerator());
        Duration::from_nanos(nanos.min(u128::from(u64::MAX)) as u64)
    }

    #[must_use]
    pub fn samples(&self, index: u64, sample_rate: u32) -> Range<u64> {
        let at = |index: u64| {
            (u128::from(index) * u128::from(sample_rate) * u128::from(self.output.denominator())
                / u128::from(self.output.numerator())) as u64
        };
        at(index)..at(index + 1)
    }
}
