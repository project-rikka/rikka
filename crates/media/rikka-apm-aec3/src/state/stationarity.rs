use crate::common::{BLOCKS_PER_SECOND, SPECTRUM_LEN, Spectrum};
use crate::ring::Ring;

const MIN_NOISE_POWER: f32 = 10.0;
const HANGOVER_BLOCKS: i32 = (BLOCKS_PER_SECOND / 20) as i32;
const BLOCKS_AVERAGE_INIT_PHASE: usize = 20;
const BLOCKS_INITIAL_PHASE: usize = BLOCKS_PER_SECOND * 2;
const WINDOW_LENGTH: usize = 13;

#[derive(Clone, Debug)]
pub(crate) struct StationarityEstimator {
    noise: NoiseSpectrum,
    hangovers: [i32; SPECTRUM_LEN],
    stationarity_flags: [bool; SPECTRUM_LEN],
}

impl StationarityEstimator {
    pub(crate) fn new() -> StationarityEstimator {
        StationarityEstimator {
            noise: NoiseSpectrum::new(),
            hangovers: [0; SPECTRUM_LEN],
            stationarity_flags: [false; SPECTRUM_LEN],
        }
    }

    pub(crate) fn update_noise_estimator(&mut self, spectrum: &[Spectrum]) {
        self.noise.update(spectrum);
    }

    pub(crate) fn update_stationarity_flags(
        &mut self,
        spectra: &Ring<Vec<Spectrum>>,
        render_reverb_contribution: &Spectrum,
        idx_current: usize,
        num_lookahead: i32,
    ) {
        let num_lookahead_bounded = num_lookahead.min(WINDOW_LENGTH as i32 - 1);
        let mut idx = idx_current;
        if num_lookahead_bounded < WINDOW_LENGTH as i32 - 1 {
            let num_lookback = (WINDOW_LENGTH as i32 - 1) - num_lookahead_bounded;
            idx = spectra.offset(idx_current, num_lookback as isize);
        }
        let mut indexes = [0; WINDOW_LENGTH];
        indexes[0] = idx;
        for k in 1..WINDOW_LENGTH {
            indexes[k] = spectra.dec(indexes[k - 1]);
        }
        for band in 0..SPECTRUM_LEN {
            self.stationarity_flags[band] = self.estimate_band_stationarity(
                spectra,
                render_reverb_contribution,
                &indexes,
                band,
            );
        }
        self.update_hangover();
        self.smooth_stationary_per_freq();
    }

    pub(crate) fn is_band_stationary(&self, band: usize) -> bool {
        self.stationarity_flags[band] && self.hangovers[band] == 0
    }

    pub(crate) fn is_block_stationary(&self) -> bool {
        let mut acum_stationarity = 0.0_f32;
        for band in 0..SPECTRUM_LEN {
            acum_stationarity += f32::from(u8::from(self.is_band_stationary(band)));
        }
        acum_stationarity * (1.0 / SPECTRUM_LEN as f32) > 0.75
    }

    fn estimate_band_stationarity(
        &self,
        spectra: &Ring<Vec<Spectrum>>,
        average_reverb: &Spectrum,
        indexes: &[usize; WINDOW_LENGTH],
        band: usize,
    ) -> bool {
        const THRESHOLD_STATIONARITY: f32 = 10.0;
        let one_by_channels = 1.0 / spectra.buffer[0].len() as f32;
        let mut acum_power = 0.0_f32;
        for idx in indexes {
            for channel in &spectra.buffer[*idx] {
                acum_power += channel[band] * one_by_channels;
            }
        }
        acum_power += average_reverb[band];
        let noise = WINDOW_LENGTH as f32 * self.noise.power(band);
        acum_power < THRESHOLD_STATIONARITY * noise
    }

    fn update_hangover(&mut self) {
        let reduce_hangover = self.stationarity_flags.iter().all(|flag| *flag);
        for (hangover, flag) in self.hangovers.iter_mut().zip(&self.stationarity_flags) {
            if !flag {
                *hangover = HANGOVER_BLOCKS;
            } else if reduce_hangover {
                *hangover = (*hangover - 1).max(0);
            }
        }
    }

    fn smooth_stationary_per_freq(&mut self) {
        let flags = &self.stationarity_flags;
        let mut smoothed = [false; SPECTRUM_LEN];
        for k in 1..SPECTRUM_LEN - 1 {
            smoothed[k] = flags[k - 1] && flags[k] && flags[k + 1];
        }
        smoothed[0] = smoothed[1];
        smoothed[SPECTRUM_LEN - 1] = smoothed[SPECTRUM_LEN - 2];
        self.stationarity_flags = smoothed;
    }
}

#[derive(Clone, Debug)]
struct NoiseSpectrum {
    noise_spectrum: Spectrum,
    block_counter: usize,
}

impl NoiseSpectrum {
    fn new() -> NoiseSpectrum {
        NoiseSpectrum {
            noise_spectrum: [MIN_NOISE_POWER; SPECTRUM_LEN],
            block_counter: 0,
        }
    }

    fn power(&self, band: usize) -> f32 {
        self.noise_spectrum[band]
    }

    fn update(&mut self, spectrum: &[Spectrum]) {
        let mut average = spectrum[0];
        if spectrum.len() > 1 {
            for channel in &spectrum[1..] {
                for k in 1..SPECTRUM_LEN {
                    average[k] += channel[k];
                }
            }
            let one_by_channels = 1.0 / spectrum.len() as f32;
            for value in &mut average[1..] {
                *value *= one_by_channels;
            }
        }
        self.block_counter += 1;
        let alpha = self.alpha();
        for k in 0..SPECTRUM_LEN {
            if self.block_counter <= BLOCKS_AVERAGE_INIT_PHASE {
                self.noise_spectrum[k] += (1.0 / BLOCKS_AVERAGE_INIT_PHASE as f32) * average[k];
            } else {
                self.noise_spectrum[k] =
                    self.update_band_by_smoothing(average[k], self.noise_spectrum[k], alpha);
            }
        }
    }

    fn alpha(&self) -> f32 {
        const ALPHA: f32 = 0.004;
        const ALPHA_INIT: f32 = 0.04;
        const TILT_ALPHA: f32 = (ALPHA_INIT - ALPHA) / BLOCKS_INITIAL_PHASE as f32;
        if self.block_counter > BLOCKS_INITIAL_PHASE + BLOCKS_AVERAGE_INIT_PHASE {
            ALPHA
        } else {
            ALPHA_INIT
                - TILT_ALPHA * self.block_counter.wrapping_sub(BLOCKS_AVERAGE_INIT_PHASE) as f32
        }
    }

    fn update_band_by_smoothing(&self, power_band: f32, power_band_noise: f32, alpha: f32) -> f32 {
        let mut updated = power_band_noise;
        if power_band_noise < power_band {
            let mut alpha_inc = alpha * (power_band_noise / power_band);
            if self.block_counter > BLOCKS_INITIAL_PHASE && 10.0 * power_band_noise < power_band {
                alpha_inc *= 0.1;
            }
            updated += alpha_inc * (power_band - power_band_noise);
        } else {
            updated += alpha * (power_band - power_band_noise);
            updated = updated.max(MIN_NOISE_POWER);
        }
        updated
    }
}
