use rikka_simd::Level;

use crate::axis::Axis;
use crate::passes::{across, down};
use crate::taps::{Footprint, Taps};

const MOST_BANDS: usize = 8;
const BANDED_FROM_TEXELS: usize = 1 << 21;
const FEWEST_BAND_ROWS: usize = 16;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Threads {
    #[default]
    Calling,
    Available,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Job {
    pub x: Axis,
    pub y: Axis,
    pub footprint: Footprint,
    pub threads: Threads,
}

#[derive(Debug, Default)]
struct Band {
    ring: Vec<f32>,
    loaded: Vec<f32>,
    blended: Vec<f32>,
}

#[derive(Debug)]
pub struct Resampler {
    level: Level,
    columns: Taps,
    rows: Taps,
    band: Band,
}

impl Default for Resampler {
    fn default() -> Resampler {
        Resampler::new()
    }
}

struct Pass<'p, const C: usize> {
    level: Level,
    columns: &'p Taps,
    rows: &'p Taps,
    source_width: usize,
    width: usize,
    pitch: usize,
}

impl<const C: usize> Pass<'_, C> {
    fn run<T>(
        &self,
        band: &mut Band,
        first_row: usize,
        target: &mut [T],
        load: &(impl Fn(usize, &mut [f32]) + Sync),
        store: &(impl Fn(&[f32], &mut [T]) + Sync),
    ) {
        let width = self.width * C;
        let depth = self.rows.widest.max(1);
        band.ring.clear();
        band.ring.resize(depth * width, 0.0);
        band.loaded.clear();
        band.loaded.resize(self.source_width * C, 0.0);
        band.blended.clear();
        band.blended.resize(width, 0.0);
        let level = self.level;
        let columns = self.columns;
        let mut next = 0;
        for (offset, out) in target.chunks_exact_mut(self.pitch).enumerate() {
            let tap = self.rows.taps[first_row + offset];
            next = next.max(tap.first);
            while next < tap.first + tap.count {
                load(next, &mut band.loaded);
                let slot = next % depth * width;
                let (loaded, kept) = (&band.loaded, &mut band.ring[slot..slot + width]);
                rikka_simd::dispatch!(level, simd => across::<_, C>(simd, loaded, columns, kept));
                next += 1;
            }
            band.blended.fill(0.0);
            for (index, &weight) in self.rows.weights_of(tap).iter().enumerate() {
                let slot = (tap.first + index) % depth * width;
                let (kept, blended) = (&band.ring[slot..slot + width], &mut band.blended);
                rikka_simd::dispatch!(level, simd => down(simd, kept, weight, blended));
            }
            store(&band.blended, out);
        }
    }
}

impl Resampler {
    #[must_use]
    pub fn new() -> Resampler {
        Resampler::with_level(Level::detect())
    }

    #[must_use]
    pub fn with_level(level: Level) -> Resampler {
        Resampler {
            level,
            columns: Taps::default(),
            rows: Taps::default(),
            band: Band::default(),
        }
    }

    pub fn resample<T: Send, const C: usize>(
        &mut self,
        job: Job,
        target: &mut [T],
        load: impl Fn(usize, &mut [f32]) + Sync,
        store: impl Fn(&[f32], &mut [T]) + Sync,
    ) {
        let Job {
            x,
            y,
            footprint,
            threads,
        } = job;
        if x.target_len == 0 || y.target_len == 0 || target.is_empty() {
            return;
        }
        self.columns.build(x, footprint);
        self.rows.build(y, footprint);
        let pass = Pass::<C> {
            level: self.level,
            columns: &self.columns,
            rows: &self.rows,
            source_width: x.source_len,
            width: x.target_len,
            pitch: target.len() / y.target_len,
        };
        let bands = match threads {
            Threads::Available if x.source_len * y.source_len >= BANDED_FROM_TEXELS => {
                std::thread::available_parallelism()
                    .map_or(1, |cores| cores.get())
                    .min(MOST_BANDS)
                    .min(y.target_len / FEWEST_BAND_ROWS)
                    .max(1)
            }
            _ => 1,
        };
        if bands == 1 {
            pass.run(&mut self.band, 0, target, &load, &store);
            return;
        }
        let band_rows = y.target_len.div_ceil(bands);
        let (first, rest) = target.split_at_mut(band_rows * pass.pitch);
        let (pass, load, store) = (&pass, &load, &store);
        std::thread::scope(|scope| {
            for (index, rows) in rest.chunks_mut(band_rows * pass.pitch).enumerate() {
                std::thread::Builder::new()
                    .name(format!("rikka-image-resample-{index}"))
                    .spawn_scoped(scope, move || {
                        let first_row = (index + 1) * band_rows;
                        pass.run(&mut Band::default(), first_row, rows, load, store);
                    })
                    .expect("failed to spawn image resampler");
            }
            pass.run(&mut self.band, 0, first, load, store);
        });
    }
}
