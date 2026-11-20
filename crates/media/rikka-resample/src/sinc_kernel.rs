use rikka_simd::{Level, Simd};

use crate::convolution::convolve;
use crate::cursor::Cursor;
use crate::interpolator::Interpolator;
use crate::kernel::Kernel;
use crate::phase_bank::PhaseBank;
use crate::planes::Planes;
use crate::sinc_params::SincParams;
use crate::sinc_table::SincTable;
use crate::weights::Weights;

const MIN_TAPS: usize = 32;
const MAX_TAPS: usize = 1024;
const MAX_STRETCHED_TAPS: usize = 2048;

const MIN_OVERSAMPLING: usize = 2;
const MAX_OVERSAMPLING: usize = 1024;

const MIN_CUTOFF: f64 = 1.0e-3;

const MAX_BANKED_WEIGHTS: usize = 1 << 17;

pub(crate) struct SincKernel {
    filter: Filter,
    taps: usize,
    level: Level,
}

enum Filter {
    Banked(PhaseBank),
    Interpolated(Interpolator),
}

impl SincKernel {
    pub(crate) fn new(params: SincParams, cursor: &Cursor) -> SincKernel {
        let squeeze = cursor.ratio().clamp(f64::MIN_POSITIVE, 1.0);
        let taps = round_up_to_eight(
            (params.taps.clamp(MIN_TAPS, MAX_TAPS) as f64 / squeeze).ceil() as usize,
        )
        .min(MAX_STRETCHED_TAPS);
        let oversampling = params
            .oversampling
            .clamp(MIN_OVERSAMPLING, MAX_OVERSAMPLING);

        let cutoff = (squeeze - (1.0 - params.window.cutoff(taps))).clamp(MIN_CUTOFF, 1.0);

        let table = SincTable::new(params.window, taps, oversampling, cutoff);
        let level = Level::detect();
        let phases = cursor.phases();
        let banked = phases.saturating_mul(table.span());
        let filter = if banked <= table.footprint().max(MAX_BANKED_WEIGHTS) {
            Filter::Banked(rikka_simd::dispatch!(
                level,
                simd => PhaseBank::new(simd, &table, phases)
            ))
        } else {
            Filter::Interpolated(Interpolator::new(table))
        };
        SincKernel {
            filter,
            taps,
            level,
        }
    }
}

impl Kernel for SincKernel {
    fn lookbehind(&self) -> usize {
        self.taps / 2
    }

    fn lookahead(&self) -> usize {
        self.taps / 2 + 1
    }

    fn render(&mut self, planes: &Planes, cursor: &mut Cursor, end: usize, output: &mut Vec<f32>) {
        let lookbehind = self.lookbehind();
        match &mut self.filter {
            Filter::Banked(bank) => rikka_simd::dispatch!(
                self.level,
                simd => render_frames(simd, bank, lookbehind, planes, cursor, end, output)
            ),
            Filter::Interpolated(interpolator) => rikka_simd::dispatch!(
                self.level,
                simd => render_frames(simd, interpolator, lookbehind, planes, cursor, end, output)
            ),
        }
    }
}

#[inline(always)]
fn render_frames<S: Simd>(
    simd: S,
    weights: &mut impl Weights,
    lookbehind: usize,
    planes: &Planes,
    cursor: &mut Cursor,
    end: usize,
    output: &mut Vec<f32>,
) {
    let (pairs, single) = planes.as_slice().as_chunks::<2>();
    while cursor.index() < end {
        let weights = weights.weights(simd, cursor);
        let start = cursor.index() - lookbehind;
        for [left, right] in pairs {
            output.extend(convolve(simd, weights, [&left[start..], &right[start..]]));
        }
        for mono in single {
            output.extend(convolve(simd, weights, [&mono[start..]]));
        }
        cursor.advance();
    }
}

fn round_up_to_eight(value: usize) -> usize {
    value.div_ceil(8) * 8
}
