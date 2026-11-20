use crate::axis::Axis;
use crate::kernel::Kernel;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Footprint {
    Filtered(Kernel),
    Coverage,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Tap {
    pub(crate) first: usize,
    pub(crate) weights: usize,
    pub(crate) count: usize,
}

#[derive(Debug, Default)]
pub(crate) struct Taps {
    pub(crate) taps: Vec<Tap>,
    pub(crate) weights: Vec<f32>,
    pub(crate) widest: usize,
}

impl Taps {
    pub(crate) fn build(&mut self, axis: Axis, footprint: Footprint) {
        self.taps.clear();
        self.weights.clear();
        match footprint {
            Footprint::Filtered(kernel) => self.filtered(axis, kernel),
            Footprint::Coverage => self.coverage(axis),
        }
        self.widest = self.taps.iter().map(|tap| tap.count).max().unwrap_or(0);
    }

    pub(crate) fn weights_of(&self, tap: Tap) -> &[f32] {
        &self.weights[tap.weights..tap.weights + tap.count]
    }

    fn filtered(&mut self, axis: Axis, kernel: Kernel) {
        let scale = f64::from(axis.scale);
        let stretch = (1.0 / scale).max(1.0);
        let reach = kernel.support() * stretch;
        let last = axis.source_len.saturating_sub(1) as f64;
        for target in 0..axis.target_len {
            let center = (target as f64 + 0.5 - f64::from(axis.origin)) / scale - 0.5;
            let low = (center - reach).ceil().clamp(0.0, last);
            let high = (center + reach).floor().clamp(low, last);
            let weights = self.weights.len();
            let mut total = 0.0;
            let mut index = low;
            while index <= high {
                let weight = kernel.weight((index - center) / stretch);
                self.weights.push(weight as f32);
                total += weight;
                index += 1.0;
            }
            let run = &mut self.weights[weights..];
            if total.abs() > f64::EPSILON {
                run.iter_mut()
                    .for_each(|weight| *weight = (f64::from(*weight) / total) as f32);
            } else {
                run.fill(0.0);
                let nearest = (center.round().clamp(low, high) - low) as usize;
                run[nearest] = 1.0;
            }
            self.taps.push(Tap {
                first: low as usize,
                weights,
                count: run.len(),
            });
        }
    }

    fn coverage(&mut self, axis: Axis) {
        let len = axis.source_len as f32;
        for target in 0..axis.target_len {
            let weights = self.weights.len();
            let position = target as f32 - axis.origin;
            let first = if axis.scale < 1.0 {
                let start = position / axis.scale;
                let end = (position + 1.0) / axis.scale;
                let low = start.floor().max(0.0);
                let high = end.ceil().min(len);
                let mut index = low;
                while index < high {
                    let overlap = end.min(index + 1.0) - start.max(index);
                    self.weights.push(overlap.max(0.0) * axis.scale);
                    index += 1.0;
                }
                low
            } else {
                let center = (position + 0.5) / axis.scale - 0.5;
                let base = center.floor();
                let fraction = center - base;
                let mut first = None;
                for (index, weight) in [(base, 1.0 - fraction), (base + 1.0, fraction)] {
                    if index >= 0.0 && index < len {
                        first.get_or_insert(index);
                        self.weights.push(weight);
                    }
                }
                first.unwrap_or(0.0)
            };
            self.taps.push(Tap {
                first: first as usize,
                weights,
                count: self.weights.len() - weights,
            });
        }
    }
}
