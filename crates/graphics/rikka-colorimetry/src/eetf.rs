use crate::transfer::{PQ_PEAK, TransferFunction};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Eetf {
    pub anchor: f64,
    pub source_peak: f64,
    pub target_peak: f64,
    pub knee: f64,
}

impl Eetf {
    pub fn new(anchor: f64, source_peak: f64, target_peak: f64) -> Option<Self> {
        if !(source_peak > target_peak && target_peak > 0.0 && anchor > 0.0) {
            return None;
        }
        let source = pq(source_peak * anchor);
        let target = pq(target_peak * anchor) / source;
        Some(Self {
            anchor,
            source_peak: source,
            target_peak: target,
            knee: (1.5 * target - 0.5).max(0.0),
        })
    }

    pub fn map(&self, relative: f64) -> f64 {
        if relative <= 0.0 {
            return 0.0;
        }
        let encoded = (pq(relative * self.anchor) / self.source_peak).min(1.0);
        if encoded <= self.knee {
            return relative;
        }
        let mapped = self.spline(encoded);
        TransferFunction::Pq.decode(mapped * self.source_peak) * PQ_PEAK / self.anchor
    }

    pub fn spline(&self, encoded: f64) -> f64 {
        let span = 1.0 - self.knee;
        let t = (encoded - self.knee) / span;
        let (t2, t3) = (t * t, t * t * t);
        (2.0 * t3 - 3.0 * t2 + 1.0) * self.knee
            + (t3 - 2.0 * t2 + t) * span
            + (-2.0 * t3 + 3.0 * t2) * self.target_peak
    }
}

fn pq(nits: f64) -> f64 {
    TransferFunction::Pq.encode(nits / PQ_PEAK)
}
