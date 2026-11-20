use crate::common::{FFT_LEN_BY_2, SPECTRUM_LEN, Spectrum, safe_clamp};
use crate::config::Config;
use crate::render::RenderBuffer;

const SUBBANDS: usize = 6;
const BAND_BOUNDARIES: [usize; SUBBANDS + 1] = [1, 8, 16, 24, 32, 48, SPECTRUM_LEN];
const X2_BAND_ENERGY_THRESHOLD: f32 = 44_015_068.0;
const SMOOTHING_DECREASES: f32 = 0.1;
const SMOOTHING_INCREASES: f32 = SMOOTHING_DECREASES / 2.0;
const NUM_UPDATE_THRESHOLD: i32 = 50;

#[derive(Clone, Debug)]
pub(crate) struct SignalDependentErleEstimator {
    min_erle: f32,
    num_sections: usize,
    band_to_subband: [usize; SPECTRUM_LEN],
    max_erle: [f32; SUBBANDS],
    section_boundaries_blocks: Vec<usize>,
    use_onset_detection: bool,
    erle: Vec<Spectrum>,
    erle_onset_compensated: Vec<Spectrum>,
    s2_section_accum: Vec<Vec<Spectrum>>,
    erle_estimators: Vec<Vec<[f32; SUBBANDS]>>,
    erle_ref: Vec<[f32; SUBBANDS]>,
    correction_factors: Vec<Vec<[f32; SUBBANDS]>>,
    num_updates: Vec<[i32; SUBBANDS]>,
    n_active_sections: Vec<[usize; SPECTRUM_LEN]>,
}

impl SignalDependentErleEstimator {
    pub(crate) fn new(config: &Config, capture_channels: usize) -> SignalDependentErleEstimator {
        let num_sections = config.erle.num_sections;
        let num_blocks = config.filter.refined.length_blocks;
        let delay_headroom_blocks = config.delay_headroom_blocks();
        let band_to_subband = subband_map();
        let limit_subband_l = band_to_subband[FFT_LEN_BY_2 / 2];
        let mut max_erle = [config.erle.max_h; SUBBANDS];
        max_erle[..limit_subband_l].fill(config.erle.max_l);
        let mut estimator = SignalDependentErleEstimator {
            min_erle: config.erle.min,
            num_sections,
            band_to_subband,
            max_erle,
            section_boundaries_blocks: section_boundaries(
                delay_headroom_blocks,
                num_blocks,
                num_sections,
            ),
            use_onset_detection: config.erle.onset_detection,
            erle: vec![[0.0; SPECTRUM_LEN]; capture_channels],
            erle_onset_compensated: vec![[0.0; SPECTRUM_LEN]; capture_channels],
            s2_section_accum: vec![vec![[0.0; SPECTRUM_LEN]; num_sections]; capture_channels],
            erle_estimators: vec![vec![[0.0; SUBBANDS]; num_sections]; capture_channels],
            erle_ref: vec![[0.0; SUBBANDS]; capture_channels],
            correction_factors: vec![vec![[0.0; SUBBANDS]; num_sections]; capture_channels],
            num_updates: vec![[0; SUBBANDS]; capture_channels],
            n_active_sections: vec![[0; SPECTRUM_LEN]; capture_channels],
        };
        estimator.reset();
        estimator
    }

    pub(crate) fn reset(&mut self) {
        for ch in 0..self.erle.len() {
            self.erle[ch] = [self.min_erle; SPECTRUM_LEN];
            self.erle_onset_compensated[ch] = [self.min_erle; SPECTRUM_LEN];
            self.erle_estimators[ch].fill([self.min_erle; SUBBANDS]);
            self.erle_ref[ch] = [self.min_erle; SUBBANDS];
            self.correction_factors[ch].fill([1.0; SUBBANDS]);
            self.num_updates[ch] = [0; SUBBANDS];
            self.n_active_sections[ch] = [0; SPECTRUM_LEN];
        }
    }

    pub(crate) fn erle(&self, onset_compensated: bool) -> &[Spectrum] {
        if onset_compensated && self.use_onset_detection {
            &self.erle_onset_compensated
        } else {
            &self.erle
        }
    }

    pub(crate) fn update(&mut self, input: &SignalDependentInput<'_>) {
        self.compute_echo_estimate_per_filter_section(
            input.render,
            input.filter_frequency_responses,
        );
        self.compute_active_filter_sections();
        self.update_correction_factors(input.x2, input.y2, input.e2, input.converged_filters);
        for ch in 0..self.erle.len() {
            for k in 0..FFT_LEN_BY_2 {
                let subband = self.band_to_subband[k];
                let correction_factor =
                    self.correction_factors[ch][self.n_active_sections[ch][k]][subband];
                self.erle[ch][k] = safe_clamp(
                    input.average_erle[ch][k] * correction_factor,
                    self.min_erle,
                    self.max_erle[subband],
                );
                if self.use_onset_detection {
                    self.erle_onset_compensated[ch][k] = safe_clamp(
                        input.average_erle_onset_compensated[ch][k] * correction_factor,
                        self.min_erle,
                        self.max_erle[subband],
                    );
                }
            }
        }
    }

    fn update_correction_factors(
        &mut self,
        x2: &Spectrum,
        y2: &[Spectrum],
        e2: &[Spectrum],
        converged_filters: &[bool],
    ) {
        for (ch, converged) in converged_filters.iter().enumerate() {
            if !converged {
                continue;
            }
            let x2_subbands = subband_powers(x2);
            let e2_subbands = subband_powers(&e2[ch]);
            let y2_subbands = subband_powers(&y2[ch]);
            let mut idx_subbands = [0; SUBBANDS];
            for (subband, idx) in idx_subbands.iter_mut().enumerate() {
                let range = BAND_BOUNDARIES[subband]..BAND_BOUNDARIES[subband + 1];
                *idx = self.n_active_sections[ch][range]
                    .iter()
                    .copied()
                    .min()
                    .unwrap_or(0);
            }
            let mut new_erle = [0.0_f32; SUBBANDS];
            let mut is_erle_updated = [false; SUBBANDS];
            for subband in 0..SUBBANDS {
                if x2_subbands[subband] > X2_BAND_ENERGY_THRESHOLD && e2_subbands[subband] > 0.0 {
                    new_erle[subband] = y2_subbands[subband] / e2_subbands[subband];
                    is_erle_updated[subband] = true;
                    self.num_updates[ch][subband] += 1;
                }
            }
            for subband in 0..SUBBANDS {
                let estimate = &mut self.erle_estimators[ch][idx_subbands[subband]][subband];
                smooth(
                    estimate,
                    new_erle[subband],
                    is_erle_updated[subband],
                    self.min_erle,
                    self.max_erle[subband],
                );
            }
            for subband in 0..SUBBANDS {
                let reference = &mut self.erle_ref[ch][subband];
                smooth(
                    reference,
                    new_erle[subband],
                    is_erle_updated[subband],
                    self.min_erle,
                    self.max_erle[subband],
                );
            }
            for subband in 0..SUBBANDS {
                if is_erle_updated[subband] && self.num_updates[ch][subband] > NUM_UPDATE_THRESHOLD
                {
                    let idx = idx_subbands[subband];
                    let new_correction_factor =
                        self.erle_estimators[ch][idx][subband] / self.erle_ref[ch][subband];
                    let factor = &mut self.correction_factors[ch][idx][subband];
                    *factor += 0.1 * (new_correction_factor - *factor);
                }
            }
        }
    }

    fn compute_echo_estimate_per_filter_section(
        &mut self,
        render: &RenderBuffer,
        filter_frequency_responses: &[Vec<Spectrum>],
    ) {
        let spectra = &render.spectra;
        let one_by_render_channels = 1.0 / spectra.buffer[0].len() as f32;
        for (capture_ch, accum) in self.s2_section_accum.iter_mut().enumerate() {
            let mut idx_render = spectra.offset(
                render.position(),
                self.section_boundaries_blocks[0] as isize,
            );
            for section in 0..self.num_sections {
                let mut x2_section = [0.0_f32; SPECTRUM_LEN];
                let mut h2_section = [0.0_f32; SPECTRUM_LEN];
                let block_limit = self.section_boundaries_blocks[section + 1]
                    .min(filter_frequency_responses[capture_ch].len());
                let first = self.section_boundaries_blocks[section];
                let blocks = filter_frequency_responses[capture_ch]
                    .get(first..block_limit)
                    .unwrap_or(&[]);
                for block in blocks {
                    for channel_spectrum in &spectra.buffer[idx_render] {
                        for (sum, value) in x2_section.iter_mut().zip(channel_spectrum) {
                            *sum += value * one_by_render_channels;
                        }
                    }
                    for (sum, value) in h2_section.iter_mut().zip(block) {
                        *sum += value;
                    }
                    idx_render = spectra.inc(idx_render);
                }
                for k in 0..SPECTRUM_LEN {
                    accum[section][k] = x2_section[k] * h2_section[k];
                }
            }
            for section in 1..self.num_sections {
                let previous = accum[section - 1];
                for (value, previous) in accum[section].iter_mut().zip(previous) {
                    *value += previous;
                }
            }
        }
    }

    fn compute_active_filter_sections(&mut self) {
        for (active, accum) in self
            .n_active_sections
            .iter_mut()
            .zip(&self.s2_section_accum)
        {
            *active = [0; SPECTRUM_LEN];
            for k in 0..SPECTRUM_LEN {
                let mut section = self.num_sections;
                let target = 0.9 * accum[self.num_sections - 1][k];
                while section > 0 && accum[section - 1][k] >= target {
                    section -= 1;
                    active[k] = section;
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SignalDependentInput<'a> {
    pub(crate) render: &'a RenderBuffer,
    pub(crate) filter_frequency_responses: &'a [Vec<Spectrum>],
    pub(crate) x2: &'a Spectrum,
    pub(crate) y2: &'a [Spectrum],
    pub(crate) e2: &'a [Spectrum],
    pub(crate) average_erle: &'a [Spectrum],
    pub(crate) average_erle_onset_compensated: &'a [Spectrum],
    pub(crate) converged_filters: &'a [bool],
}

fn smooth(value: &mut f32, new_erle: f32, updated: bool, min_erle: f32, max_erle: f32) {
    let mut alpha = if new_erle > *value {
        SMOOTHING_INCREASES
    } else {
        SMOOTHING_DECREASES
    };
    alpha *= f32::from(u8::from(updated));
    *value += alpha * (new_erle - *value);
    *value = safe_clamp(*value, min_erle, max_erle);
}

fn subband_powers(spectrum: &Spectrum) -> [f32; SUBBANDS] {
    let mut powers = [0.0; SUBBANDS];
    for (subband, power) in powers.iter_mut().enumerate() {
        *power =
            crate::common::sum(&spectrum[BAND_BOUNDARIES[subband]..BAND_BOUNDARIES[subband + 1]]);
    }
    powers
}

fn subband_map() -> [usize; SPECTRUM_LEN] {
    let mut map = [0; SPECTRUM_LEN];
    let mut subband = 1;
    for (k, value) in map.iter_mut().enumerate() {
        if k >= BAND_BOUNDARIES[subband] {
            subband += 1;
        }
        *value = subband - 1;
    }
    map
}

fn section_sizes(
    delay_headroom_blocks: usize,
    num_blocks: usize,
    num_sections: usize,
) -> Vec<usize> {
    let filter_length_blocks = num_blocks - delay_headroom_blocks;
    let mut sizes = vec![0; num_sections];
    let mut remaining_blocks = filter_length_blocks;
    let mut remaining_sections = num_sections;
    let mut estimator_size = 2;
    let mut idx = 0;
    while remaining_sections > 1 && remaining_blocks > estimator_size * remaining_sections {
        sizes[idx] = estimator_size;
        remaining_blocks -= estimator_size;
        remaining_sections -= 1;
        estimator_size *= 2;
        idx += 1;
    }
    let last_groups_size = remaining_blocks / remaining_sections;
    for size in &mut sizes[idx..] {
        *size = last_groups_size;
    }
    sizes[num_sections - 1] += remaining_blocks - last_groups_size * remaining_sections;
    sizes
}

fn section_boundaries(
    delay_headroom_blocks: usize,
    num_blocks: usize,
    num_sections: usize,
) -> Vec<usize> {
    let mut boundaries = vec![0; num_sections + 1];
    if boundaries.len() == 2 {
        boundaries[1] = num_blocks;
        return boundaries;
    }
    let sizes = section_sizes(delay_headroom_blocks, num_blocks, num_sections);
    let mut idx = 0;
    let mut current_size_block = 0;
    boundaries[0] = delay_headroom_blocks;
    for k in delay_headroom_blocks..num_blocks {
        current_size_block += 1;
        if current_size_block >= sizes[idx] {
            idx += 1;
            if idx == sizes.len() {
                break;
            }
            boundaries[idx] = k + 1;
            current_size_block = 0;
        }
    }
    boundaries[sizes.len()] = num_blocks;
    boundaries
}
