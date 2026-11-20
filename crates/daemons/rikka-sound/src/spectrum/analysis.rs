use std::f32::consts::TAU;

use rikka_fft::RealFft;

const SIZE: usize = 2048;
const LOWEST: f32 = 50.0;
const HIGHEST: f32 = 16_000.0;
const SHY_OF_NYQUIST: f32 = 0.95;
const FLOOR: f32 = -66.0;
const CEILING: f32 = -12.0;
const TILT_PER_OCTAVE: f32 = 3.0;
const PIVOT: f32 = 1_000.0;
const ANALYSES_PER_SECOND: u32 = 60;

struct Band {
    from: usize,
    to: usize,
    tilt: f32,
}

pub(super) struct Analysis {
    fft: RealFft,
    window: Vec<f32>,
    gain: f32,
    ring: Vec<f32>,
    at: usize,
    unheard: usize,
    hop: usize,
    windowed: Vec<f32>,
    spectrum: Vec<f32>,
    bands: Vec<Band>,
    levels: Vec<f32>,
}

impl Analysis {
    pub(super) fn new(bands: usize, rate: u32) -> Analysis {
        let window: Vec<f32> = (0..SIZE)
            .map(|index| 0.5 - 0.5 * (TAU * index as f32 / SIZE as f32).cos())
            .collect();
        let gain = 2.0 / window.iter().sum::<f32>();
        Analysis {
            fft: RealFft::new(SIZE),
            window,
            gain,
            ring: vec![0.0; SIZE],
            at: 0,
            unheard: 0,
            hop: (rate / ANALYSES_PER_SECOND).max(1) as usize,
            windowed: vec![0.0; SIZE],
            spectrum: vec![0.0; SIZE],
            bands: banded(bands, rate),
            levels: vec![0.0; bands],
        }
    }

    pub(super) fn hear(&mut self, samples: impl IntoIterator<Item = f32>) -> Option<&[f32]> {
        for sample in samples {
            self.ring[self.at] = sample;
            self.at = (self.at + 1) % SIZE;
            self.unheard += 1;
        }
        if self.unheard < self.hop {
            return None;
        }
        self.unheard = 0;
        self.analyse();
        Some(&self.levels)
    }

    fn analyse(&mut self) {
        for (index, sample) in self.windowed.iter_mut().enumerate() {
            *sample = self.ring[(self.at + index) % SIZE] * self.window[index];
        }
        self.fft.forward_ordered(&self.windowed, &mut self.spectrum);
        let bins = self.spectrum.as_chunks::<2>().0;
        for (band, level) in self.bands.iter().zip(&mut self.levels) {
            let peak = bins[band.from..band.to]
                .iter()
                .map(|[real, imaginary]| real.hypot(*imaginary))
                .fold(0.0, f32::max);
            let loudness = 20.0 * (peak * self.gain).max(f32::MIN_POSITIVE).log10() + band.tilt;
            *level = ((loudness - FLOOR) / (CEILING - FLOOR)).clamp(0.0, 1.0);
        }
    }
}

fn banded(count: usize, rate: u32) -> Vec<Band> {
    let highest = HIGHEST.min(rate as f32 / 2.0 * SHY_OF_NYQUIST);
    let span = highest / LOWEST;
    let bin = |hertz: f32| hertz * SIZE as f32 / rate as f32;
    (0..count)
        .map(|index| {
            let low = LOWEST * span.powf(index as f32 / count as f32);
            let high = LOWEST * span.powf((index + 1) as f32 / count as f32);
            let from = (bin(low).floor() as usize).clamp(1, SIZE / 2 - 1);
            let to = (bin(high).ceil() as usize).clamp(from + 1, SIZE / 2);
            Band {
                from,
                to,
                tilt: TILT_PER_OCTAVE * ((low * high).sqrt() / PIVOT).log2(),
            }
        })
        .collect()
}
