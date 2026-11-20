use crate::block::Block;
use crate::common::{BLOCK_LEN, FFT_LEN_BY_2, SPECTRUM_LEN, Spectrum};
use crate::config::{Config, Tuning};
use crate::render_signal_analyzer::RenderSignalAnalyzer;
use crate::state::AecState;
use crate::suppression::moving_average::MovingAverage;
use crate::suppression::nearend_detector::{NearendDetector, NearendInput};

#[derive(Clone, Copy, Debug)]
pub(crate) struct GainInput<'a> {
    pub(crate) nearend_spectrum: &'a [Spectrum],
    pub(crate) echo_spectrum: &'a [Spectrum],
    pub(crate) residual_echo_spectrum: &'a [Spectrum],
    pub(crate) residual_echo_spectrum_unbounded: &'a [Spectrum],
    pub(crate) comfort_noise_spectrum: &'a [Spectrum],
    pub(crate) render_signal_analyzer: &'a RenderSignalAnalyzer,
    pub(crate) aec_state: &'a AecState,
    pub(crate) render: &'a Block,
    pub(crate) clock_drift: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct SuppressionGain {
    config: Config,
    last_gain: Spectrum,
    last_nearend: Vec<Spectrum>,
    last_echo: Vec<Spectrum>,
    low_render_detector: LowNoiseRenderDetector,
    initial_state: bool,
    nearend_smoothers: Vec<MovingAverage>,
    nearend_params: GainParameters,
    normal_params: GainParameters,
    use_unbounded_echo_spectrum: bool,
    dominant_nearend_detector: NearendDetector,
}

impl SuppressionGain {
    pub(crate) fn new(config: &Config, capture_channels: usize) -> SuppressionGain {
        let suppressor = &config.suppressor;
        SuppressionGain {
            config: config.clone(),
            last_gain: [1.0; SPECTRUM_LEN],
            last_nearend: vec![[0.0; SPECTRUM_LEN]; capture_channels],
            last_echo: vec![[0.0; SPECTRUM_LEN]; capture_channels],
            low_render_detector: LowNoiseRenderDetector {
                average_power: 32_768.0 * 32_768.0,
            },
            initial_state: true,
            nearend_smoothers: vec![
                MovingAverage::new(suppressor.nearend_average_blocks);
                capture_channels
            ],
            nearend_params: GainParameters::new(
                suppressor.last_lf_band,
                suppressor.first_hf_band,
                &suppressor.nearend_tuning,
            ),
            normal_params: GainParameters::new(
                suppressor.last_lf_band,
                suppressor.first_hf_band,
                &suppressor.normal_tuning,
            ),
            use_unbounded_echo_spectrum: suppressor
                .dominant_nearend_detection
                .use_unbounded_echo_spectrum,
            dominant_nearend_detector: NearendDetector::new(suppressor, capture_channels),
        }
    }

    pub(crate) fn is_dominant_nearend(&self) -> bool {
        self.dominant_nearend_detector.is_nearend_state()
    }

    pub(crate) fn set_initial_state(&mut self, state: bool) {
        self.initial_state = state;
    }

    pub(crate) fn gain(&mut self, input: &GainInput<'_>, low_band_gain: &mut Spectrum) -> f32 {
        let echo = if self.use_unbounded_echo_spectrum {
            input.residual_echo_spectrum_unbounded
        } else {
            input.residual_echo_spectrum
        };
        self.dominant_nearend_detector.update(&NearendInput {
            nearend_spectrum: input.nearend_spectrum,
            residual_echo_spectrum: echo,
            comfort_noise_spectrum: input.comfort_noise_spectrum,
            initial_state: self.initial_state,
        });
        let low_noise_render = self.low_render_detector.detect(input.render);
        self.lower_band_gain(low_noise_render, input, low_band_gain);
        self.upper_bands_gain(input, low_band_gain)
    }

    fn params(&self) -> &GainParameters {
        if self.is_dominant_nearend() {
            &self.nearend_params
        } else {
            &self.normal_params
        }
    }

    fn upper_bands_gain(&self, input: &GainInput<'_>, low_band_gain: &Spectrum) -> f32 {
        let render = input.render;
        if render.bands() == 1 {
            return 1.0;
        }
        if input
            .render_signal_analyzer
            .narrow_peak_band()
            .is_some_and(|band| band > SPECTRUM_LEN - 10)
        {
            return 0.001;
        }
        const LOW_BAND_GAIN_LIMIT: usize = FFT_LEN_BY_2 / 2;
        let gain_below_8_khz = low_band_gain[LOW_BAND_GAIN_LIMIT..]
            .iter()
            .copied()
            .fold(low_band_gain[LOW_BAND_GAIN_LIMIT], f32::min);
        if input.aec_state.saturated_echo() {
            return 0.001_f32.min(gain_below_8_khz);
        }

        let sum_of_squares = crate::common::sum_of_squares;
        let mut low_band_energy = 0.0_f32;
        for ch in 0..render.channels() {
            low_band_energy = low_band_energy.max(sum_of_squares(render.view(0, ch)));
        }
        let mut high_band_energy = 0.0_f32;
        for band in 1..render.bands() {
            for ch in 0..render.channels() {
                high_band_energy = high_band_energy.max(sum_of_squares(render.view(band, ch)));
            }
        }

        let high_bands = &self.config.suppressor.high_bands_suppression;
        let activation_threshold = BLOCK_LEN as f32 * high_bands.anti_howling_activation_threshold;
        let anti_howling_gain = if high_band_energy < low_band_energy.max(activation_threshold) {
            1.0
        } else {
            high_bands.anti_howling_gain * (low_band_energy / high_band_energy).sqrt()
        };

        let mut gain_bound = 1.0_f32;
        if !self.is_dominant_nearend() {
            let low_frequency_energy = |spectrum: &Spectrum| crate::common::sum(&spectrum[1..16]);
            for (echo, noise) in input.echo_spectrum.iter().zip(input.comfort_noise_spectrum) {
                if low_frequency_energy(echo)
                    > high_bands.enr_threshold * low_frequency_energy(noise)
                {
                    gain_bound = high_bands.max_gain_during_echo;
                    break;
                }
            }
        }
        gain_below_8_khz.min(anti_howling_gain).min(gain_bound)
    }

    fn gain_to_no_audible_echo(
        &self,
        nearend: &Spectrum,
        echo: &Spectrum,
        masker: &Spectrum,
        gain: &mut Spectrum,
    ) {
        let p = self.params();
        for k in 0..SPECTRUM_LEN {
            let enr = echo[k] / (nearend[k] + 1.0);
            let emr = echo[k] / (masker[k] + 1.0);
            let mut g = 1.0_f32;
            if enr > p.enr_transparent[k] && emr > p.emr_transparent[k] {
                g = (p.enr_suppress[k] - enr) / (p.enr_suppress[k] - p.enr_transparent[k]);
                g = g.max(p.emr_transparent[k] / emr);
            }
            gain[k] = g;
        }
    }

    fn min_gain(
        &self,
        weighted_residual_echo: &Spectrum,
        ch: usize,
        low_noise_render: bool,
        saturated_echo: bool,
    ) -> Spectrum {
        if saturated_echo {
            return [0.0; SPECTRUM_LEN];
        }
        let audibility = &self.config.echo_audibility;
        let suppressor = &self.config.suppressor;
        let min_echo_power = if low_noise_render {
            audibility.low_render_limit
        } else {
            audibility.normal_render_limit
        };
        let mut min_gain = [0.0; SPECTRUM_LEN];
        for (min_gain, echo) in min_gain.iter_mut().zip(weighted_residual_echo) {
            *min_gain = if *echo > 0.0 {
                min_echo_power / echo
            } else {
                1.0
            };
            *min_gain = min_gain.min(1.0);
        }
        if !self.initial_state || suppressor.lf_smoothing_during_initial_phase {
            let dec = self.params().max_dec_factor_lf;
            let (last_nearend, last_echo) = (&self.last_nearend[ch], &self.last_echo[ch]);
            for k in 0..=suppressor.last_lf_smoothing_band.max(-1) {
                let band = k as usize;
                if last_nearend[band] > last_echo[band]
                    || k <= suppressor.last_permanent_lf_smoothing_band
                {
                    min_gain[band] = min_gain[band].max(self.last_gain[band] * dec);
                    min_gain[band] = min_gain[band].min(1.0);
                }
            }
        }
        min_gain
    }

    fn max_gain(&self) -> Spectrum {
        let inc = self.params().max_inc_factor;
        let floor = self.config.suppressor.floor_first_increase;
        let mut max_gain = [0.0; SPECTRUM_LEN];
        for (max_gain, last) in max_gain.iter_mut().zip(&self.last_gain) {
            *max_gain = (last * inc).max(floor).min(1.0);
        }
        max_gain
    }

    fn lower_band_gain(
        &mut self,
        low_noise_render: bool,
        input: &GainInput<'_>,
        gain: &mut Spectrum,
    ) {
        *gain = [1.0; SPECTRUM_LEN];
        let saturated_echo = input.aec_state.saturated_echo();
        let max_gain = self.max_gain();
        for ch in 0..self.last_nearend.len() {
            let mut nearend = [0.0; SPECTRUM_LEN];
            self.nearend_smoothers[ch].average(&input.nearend_spectrum[ch], &mut nearend);
            let mut weighted_residual_echo = [0.0; SPECTRUM_LEN];
            weight_echo_for_audibility(
                &self.config,
                &input.residual_echo_spectrum[ch],
                &mut weighted_residual_echo,
            );
            let min_gain = self.min_gain(
                &weighted_residual_echo,
                ch,
                low_noise_render,
                saturated_echo,
            );
            let mut g = [0.0; SPECTRUM_LEN];
            self.gain_to_no_audible_echo(
                &nearend,
                &weighted_residual_echo,
                &input.comfort_noise_spectrum[0],
                &mut g,
            );
            for k in 0..SPECTRUM_LEN {
                g[k] = g[k].min(max_gain[k]).max(min_gain[k]);
                gain[k] = gain[k].min(g[k]);
            }
            self.last_nearend[ch] = nearend;
            self.last_echo[ch] = weighted_residual_echo;
        }

        gain[0] = gain[1].min(gain[2]);
        gain[1] = gain[0];
        let suppressor = &self.config.suppressor;
        if !self.is_dominant_nearend()
            || input.clock_drift
            || suppressor.conservative_hf_suppression
        {
            limit_high_frequency_gains(suppressor.conservative_hf_suppression, gain);
        }
        self.last_gain = *gain;
        for value in gain.iter_mut() {
            *value = value.sqrt();
        }
    }
}

fn limit_high_frequency_gains(conservative_hf_suppression: bool, gain: &mut Spectrum) {
    const FIRST_BAND_TO_LIMIT: usize = (64 * 2000) / 8000;
    let min_upper_gain = gain[FIRST_BAND_TO_LIMIT];
    for value in &mut gain[FIRST_BAND_TO_LIMIT + 1..] {
        *value = value.min(min_upper_gain);
    }
    gain[FFT_LEN_BY_2] = gain[FFT_LEN_BY_2 - 1];
    if conservative_hf_suppression {
        const UPPER_ACCURATE_BAND_PLUS_1: usize = 29;
        const ONE_BY_BANDS_IN_SUM: f32 = 1.0 / (UPPER_ACCURATE_BAND_PLUS_1 - 20) as f32;
        let hf_gain_bound =
            crate::common::sum(&gain[20..UPPER_ACCURATE_BAND_PLUS_1]) * ONE_BY_BANDS_IN_SUM;
        for value in &mut gain[UPPER_ACCURATE_BAND_PLUS_1..] {
            *value = value.min(hf_gain_bound);
        }
    }
}

fn weight_echo_for_audibility(config: &Config, echo: &Spectrum, weighted_echo: &mut Spectrum) {
    let audibility = &config.echo_audibility;
    let weigh =
        |threshold_factor: f32, range: std::ops::Range<usize>, weighted_echo: &mut Spectrum| {
            let threshold = audibility.floor_power * threshold_factor;
            let normalizer = 1.0 / (threshold - audibility.floor_power);
            for k in range {
                if echo[k] < threshold {
                    let tmp = (threshold - echo[k]) * normalizer;
                    weighted_echo[k] = echo[k] * 0.0_f32.max(1.0 - tmp * tmp);
                } else {
                    weighted_echo[k] = echo[k];
                }
            }
        };
    weigh(audibility.audibility_threshold_lf, 0..3, weighted_echo);
    weigh(audibility.audibility_threshold_mf, 3..7, weighted_echo);
    weigh(
        audibility.audibility_threshold_hf,
        7..SPECTRUM_LEN,
        weighted_echo,
    );
}

#[derive(Clone, Debug)]
struct LowNoiseRenderDetector {
    average_power: f32,
}

impl LowNoiseRenderDetector {
    fn detect(&mut self, render: &Block) -> bool {
        let mut x2_sum = 0.0_f32;
        let mut x2_max = 0.0_f32;
        for ch in 0..render.channels() {
            for x in render.view(0, ch) {
                let x2 = x * x;
                x2_sum += x2;
                x2_max = x2_max.max(x2);
            }
        }
        x2_sum /= render.channels() as f32;
        const THRESHOLD: f32 = 50.0 * 50.0 * 64.0;
        let low_noise_render = self.average_power < THRESHOLD && x2_max < 3.0 * self.average_power;
        self.average_power = self.average_power * 0.9 + x2_sum * 0.1;
        low_noise_render
    }
}

#[derive(Clone, Debug)]
struct GainParameters {
    max_inc_factor: f32,
    max_dec_factor_lf: f32,
    enr_transparent: Spectrum,
    enr_suppress: Spectrum,
    emr_transparent: Spectrum,
}

impl GainParameters {
    fn new(last_lf_band: i32, first_hf_band: i32, tuning: &Tuning) -> GainParameters {
        let (lf, hf) = (&tuning.mask_lf, &tuning.mask_hf);
        let mut params = GainParameters {
            max_inc_factor: tuning.max_inc_factor,
            max_dec_factor_lf: tuning.max_dec_factor_lf,
            enr_transparent: [0.0; SPECTRUM_LEN],
            enr_suppress: [0.0; SPECTRUM_LEN],
            emr_transparent: [0.0; SPECTRUM_LEN],
        };
        for k in 0..SPECTRUM_LEN as i32 {
            let a = if k <= last_lf_band {
                0.0
            } else if k < first_hf_band {
                (k - last_lf_band) as f32 / (first_hf_band - last_lf_band) as f32
            } else {
                1.0
            };
            let band = k as usize;
            params.enr_transparent[band] = (1.0 - a) * lf.enr_transparent + a * hf.enr_transparent;
            params.enr_suppress[band] = (1.0 - a) * lf.enr_suppress + a * hf.enr_suppress;
            params.emr_transparent[band] = (1.0 - a) * lf.emr_transparent + a * hf.emr_transparent;
        }
        params
    }
}
