use crate::common::{FFT_LEN_BY_2, SPECTRUM_LEN, Spectrum, safe_clamp};
use crate::config::Config;

const X2_BAND_ENERGY_THRESHOLD: f32 = 44_015_068.0;
const BLOCKS_TO_HOLD_ERLE: i32 = 100;
const BLOCKS_FOR_ONSET_DETECTION: i32 = BLOCKS_TO_HOLD_ERLE + 150;
const POINTS_TO_ACCUMULATE: i32 = 6;
const UNBOUNDED_ERLE_MAX: f32 = 100_000.0;

#[derive(Clone, Debug)]
struct Accumulated {
    y2: Spectrum,
    e2: Spectrum,
    low_render_energy: [bool; SPECTRUM_LEN],
    points: i32,
}

impl Default for Accumulated {
    fn default() -> Accumulated {
        Accumulated {
            y2: [0.0; SPECTRUM_LEN],
            e2: [0.0; SPECTRUM_LEN],
            low_render_energy: [false; SPECTRUM_LEN],
            points: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SubbandErleEstimator {
    use_onset_detection: bool,
    min_erle: f32,
    max_erle: Spectrum,
    accumulated: Vec<Accumulated>,
    erle: Vec<Spectrum>,
    erle_onset_compensated: Vec<Spectrum>,
    erle_unbounded: Vec<Spectrum>,
    erle_during_onsets: Vec<Spectrum>,
    hold_counters: Vec<[i32; SPECTRUM_LEN]>,
}

impl SubbandErleEstimator {
    pub(crate) fn new(config: &Config, capture_channels: usize) -> SubbandErleEstimator {
        let mut max_erle = [config.erle.max_h; SPECTRUM_LEN];
        max_erle[..FFT_LEN_BY_2 / 2].fill(config.erle.max_l);
        let min = [config.erle.min; SPECTRUM_LEN];
        SubbandErleEstimator {
            use_onset_detection: config.erle.onset_detection,
            min_erle: config.erle.min,
            max_erle,
            accumulated: vec![Accumulated::default(); capture_channels],
            erle: vec![min; capture_channels],
            erle_onset_compensated: vec![min; capture_channels],
            erle_unbounded: vec![min; capture_channels],
            erle_during_onsets: vec![min; capture_channels],
            hold_counters: vec![[0; SPECTRUM_LEN]; capture_channels],
        }
    }

    pub(crate) fn reset(&mut self) {
        let min = [self.min_erle; SPECTRUM_LEN];
        for ch in 0..self.erle.len() {
            self.erle[ch] = min;
            self.erle_onset_compensated[ch] = min;
            self.erle_unbounded[ch] = min;
            self.erle_during_onsets[ch] = min;
            self.hold_counters[ch] = [0; SPECTRUM_LEN];
            self.accumulated[ch] = Accumulated::default();
        }
    }

    pub(crate) fn erle(&self, onset_compensated: bool) -> &[Spectrum] {
        if onset_compensated && self.use_onset_detection {
            &self.erle_onset_compensated
        } else {
            &self.erle
        }
    }

    pub(crate) fn erle_unbounded(&self) -> &[Spectrum] {
        &self.erle_unbounded
    }

    pub(crate) fn update(
        &mut self,
        x2: &Spectrum,
        y2: &[Spectrum],
        e2: &[Spectrum],
        converged_filters: &[bool],
    ) {
        for ch in 0..self.erle.len() {
            if converged_filters[ch] {
                self.accumulate(ch, x2, &y2[ch], &e2[ch]);
            }
        }
        for (ch, converged) in converged_filters.iter().enumerate().take(self.erle.len()) {
            if *converged {
                self.update_bands(ch);
            }
        }
        if self.use_onset_detection {
            for ch in 0..self.erle.len() {
                self.decrease_erle_for_low_render_signals(ch);
            }
        }
        for ch in 0..self.erle.len() {
            for erle in [
                &mut self.erle[ch],
                &mut self.erle_onset_compensated[ch],
                &mut self.erle_unbounded[ch],
            ] {
                erle[0] = erle[1];
                erle[FFT_LEN_BY_2] = erle[FFT_LEN_BY_2 - 1];
            }
        }
    }

    fn accumulate(&mut self, ch: usize, x2: &Spectrum, y2: &Spectrum, e2: &Spectrum) {
        let accumulated = &mut self.accumulated[ch];
        if accumulated.points == POINTS_TO_ACCUMULATE {
            *accumulated = Accumulated::default();
        }
        for k in 0..SPECTRUM_LEN {
            accumulated.y2[k] += y2[k];
            accumulated.e2[k] += e2[k];
            accumulated.low_render_energy[k] =
                accumulated.low_render_energy[k] || x2[k] < X2_BAND_ENERGY_THRESHOLD;
        }
        accumulated.points += 1;
    }

    fn update_bands(&mut self, ch: usize) {
        let accumulated = &self.accumulated[ch];
        if accumulated.points != POINTS_TO_ACCUMULATE {
            return;
        }
        let mut new_erle = [0.0; FFT_LEN_BY_2];
        let mut is_erle_updated = [false; FFT_LEN_BY_2];
        for k in 1..FFT_LEN_BY_2 {
            if accumulated.e2[k] > 0.0 {
                new_erle[k] = accumulated.y2[k] / accumulated.e2[k];
                is_erle_updated[k] = true;
            }
        }
        if self.use_onset_detection {
            for k in 1..FFT_LEN_BY_2 {
                if is_erle_updated[k] && !accumulated.low_render_energy[k] {
                    self.hold_counters[ch][k] = BLOCKS_FOR_ONSET_DETECTION;
                }
            }
        }
        for k in 1..FFT_LEN_BY_2 {
            if !is_erle_updated[k] {
                continue;
            }
            let low_render_energy = accumulated.low_render_energy[k];
            let (min, max) = (self.min_erle, self.max_erle[k]);
            update_erle_band(
                &mut self.erle[ch][k],
                new_erle[k],
                low_render_energy,
                min,
                max,
            );
            if self.use_onset_detection {
                update_erle_band(
                    &mut self.erle_onset_compensated[ch][k],
                    new_erle[k],
                    low_render_energy,
                    min,
                    max,
                );
            }
            update_erle_band(
                &mut self.erle_unbounded[ch][k],
                new_erle[k],
                low_render_energy,
                min,
                UNBOUNDED_ERLE_MAX,
            );
        }
    }

    fn decrease_erle_for_low_render_signals(&mut self, ch: usize) {
        let hold_counters = &mut self.hold_counters[ch];
        let compensated = &mut self.erle_onset_compensated[ch];
        let during_onsets = &self.erle_during_onsets[ch];
        for k in 1..FFT_LEN_BY_2 {
            hold_counters[k] -= 1;
            if hold_counters[k] <= BLOCKS_FOR_ONSET_DETECTION - BLOCKS_TO_HOLD_ERLE {
                if compensated[k] > during_onsets[k] {
                    compensated[k] = during_onsets[k].max(0.97 * compensated[k]);
                }
                if hold_counters[k] <= 0 {
                    hold_counters[k] = 0;
                }
            }
        }
    }
}

fn update_erle_band(
    erle: &mut f32,
    new_erle: f32,
    low_render_energy: bool,
    min_erle: f32,
    max_erle: f32,
) {
    let mut alpha = 0.05;
    if new_erle < *erle {
        alpha = if low_render_energy { 0.0 } else { 0.1 };
    }
    *erle = safe_clamp(*erle + alpha * (new_erle - *erle), min_erle, max_erle);
}
