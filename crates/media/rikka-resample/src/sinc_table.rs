use crate::lagrange;
use crate::phases::{Phases, STENCIL};
use crate::window::WindowFunction;

const LEAD: usize = 1;
const MAX_SHIFT: usize = STENCIL - LEAD - 1;

pub(crate) struct SincTable {
    rows: Vec<f32>,
    span: usize,
    oversampling: usize,
}

impl SincTable {
    pub(crate) fn new(
        window: WindowFunction,
        taps: usize,
        oversampling: usize,
        cutoff: f64,
    ) -> SincTable {
        let span = taps + MAX_SHIFT;
        let mut rows = vec![0.0; (oversampling + STENCIL - 1) * span];
        let mut filter = vec![0.0; taps];
        for (row, placed) in rows.chunks_exact_mut(span).enumerate() {
            let wrapped = row + oversampling - LEAD;
            let offset = (wrapped % oversampling) as f64 / oversampling as f64;
            normalized_filter(&mut filter, window, offset, cutoff);
            let shift = wrapped / oversampling;
            for (target, &value) in placed[shift..].iter_mut().zip(&filter) {
                *target = value as f32;
            }
        }
        SincTable {
            rows,
            span,
            oversampling,
        }
    }

    pub(crate) fn span(&self) -> usize {
        self.span
    }

    pub(crate) fn footprint(&self) -> usize {
        self.rows.len()
    }

    #[inline(always)]
    pub(crate) fn phases(&self, frac: f64) -> Phases<'_> {
        let position = frac * self.oversampling as f64;
        let phase = position.floor();
        let stencil = &self.rows[phase as usize * self.span..][..STENCIL * self.span];
        Phases {
            rows: std::array::from_fn(|row| &stencil[row * self.span..][..self.span]),
            coefficients: lagrange::cubic(position - phase),
        }
    }
}

fn normalized_filter(filter: &mut [f64], window: WindowFunction, offset: f64, cutoff: f64) {
    let half = (filter.len() / 2) as f64;
    for (tap, value) in filter.iter_mut().enumerate() {
        let u = tap as f64 - half + 1.0 - offset;
        *value = window.evaluate(u, half) * sinc(u * cutoff);
    }
    let sum: f64 = filter.iter().sum();
    if sum.abs() > f64::EPSILON {
        let scale = 1.0 / sum;
        for value in filter {
            *value *= scale;
        }
    }
}

fn sinc(x: f64) -> f64 {
    if x == 0.0 {
        1.0
    } else {
        let x = std::f64::consts::PI * x;
        x.sin() / x
    }
}
