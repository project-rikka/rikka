use crate::block::Block;
use crate::common::{BLOCK_LEN, SPECTRUM_LEN, Spectrum, log2_to_db};
use crate::config::Config;
use crate::delay::DelayEstimate;
use crate::echo_path_variability::EchoPathVariability;
use crate::fft::{self, FftData};
use crate::filter::{Subtractor, SubtractorOutput};
use crate::render::RenderBuffer;
use crate::render_signal_analyzer::RenderSignalAnalyzer;
use crate::state::{AecState, AecStateInput};
use crate::suppression::{
    ComfortNoiseGenerator, GainInput, ResidualEchoEstimator, SuppressionFilter, SuppressionGain,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EchoMetrics {
    pub(crate) echo_return_loss: f64,
    pub(crate) echo_return_loss_enhancement: f64,
}

#[derive(Clone, Debug)]
struct Scratch {
    e: Vec<[f32; BLOCK_LEN]>,
    y2: Vec<Spectrum>,
    e2: Vec<Spectrum>,
    r2: Vec<Spectrum>,
    r2_unbounded: Vec<Spectrum>,
    s2_linear: Vec<Spectrum>,
    y: Vec<FftData>,
    e_fft: Vec<FftData>,
    comfort_noise: Vec<FftData>,
    high_band_comfort_noise: Vec<FftData>,
    subtractor_output: Vec<SubtractorOutput>,
}

impl Scratch {
    fn new(channels: usize) -> Scratch {
        Scratch {
            e: vec![[0.0; BLOCK_LEN]; channels],
            y2: vec![[0.0; SPECTRUM_LEN]; channels],
            e2: vec![[0.0; SPECTRUM_LEN]; channels],
            r2: vec![[0.0; SPECTRUM_LEN]; channels],
            r2_unbounded: vec![[0.0; SPECTRUM_LEN]; channels],
            s2_linear: vec![[0.0; SPECTRUM_LEN]; channels],
            y: vec![FftData::default(); channels],
            e_fft: vec![FftData::default(); channels],
            comfort_noise: vec![FftData::default(); channels],
            high_band_comfort_noise: vec![FftData::default(); channels],
            subtractor_output: vec![SubtractorOutput::default(); channels],
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct EchoRemover {
    has_clock_drift: bool,
    use_coarse_filter_output: bool,
    subtractor: Subtractor,
    suppression_gain: SuppressionGain,
    cng: ComfortNoiseGenerator,
    suppression_filter: SuppressionFilter,
    render_signal_analyzer: RenderSignalAnalyzer,
    residual_echo_estimator: ResidualEchoEstimator,
    aec_state: AecState,
    e_old: Vec<[f32; BLOCK_LEN]>,
    y_old: Vec<[f32; BLOCK_LEN]>,
    gain_change_hangover: i32,
    refined_filter_output_last_selected: bool,
    scratch: Scratch,
}

impl EchoRemover {
    pub(crate) fn new(
        config: &Config,
        bands: usize,
        render_channels: usize,
        capture_channels: usize,
    ) -> EchoRemover {
        EchoRemover {
            has_clock_drift: config.echo_removal_control.has_clock_drift,
            use_coarse_filter_output: config.filter.enable_coarse_filter_output_usage,
            subtractor: Subtractor::new(config, render_channels, capture_channels),
            suppression_gain: SuppressionGain::new(config, capture_channels),
            cng: ComfortNoiseGenerator::new(
                config.comfort_noise.noise_floor_dbfs,
                capture_channels,
            ),
            suppression_filter: SuppressionFilter::new(bands, capture_channels),
            render_signal_analyzer: RenderSignalAnalyzer::new(config.filter.refined.length_blocks),
            residual_echo_estimator: ResidualEchoEstimator::new(
                &config.echo_model,
                &config.ep_strength,
                render_channels,
            ),
            aec_state: AecState::new(config, capture_channels),
            e_old: vec![[0.0; BLOCK_LEN]; capture_channels],
            y_old: vec![[0.0; BLOCK_LEN]; capture_channels],
            gain_change_hangover: 0,
            refined_filter_output_last_selected: true,
            scratch: Scratch::new(capture_channels),
        }
    }

    pub(crate) fn metrics(&self) -> EchoMetrics {
        EchoMetrics {
            echo_return_loss: -10.0 * f64::from(self.aec_state.erl_time_domain().log10()),
            echo_return_loss_enhancement: f64::from(log2_to_db(
                self.aec_state.fullband_erle_log2(),
            )),
        }
    }

    pub(crate) fn process_capture(
        &mut self,
        mut variability: EchoPathVariability,
        capture_signal_saturation: bool,
        external_delay: Option<DelayEstimate>,
        render: &RenderBuffer,
        capture: &mut Block,
    ) {
        self.aec_state
            .update_capture_saturation(capture_signal_saturation);
        if variability.audio_path_changed() {
            if variability.gain_change {
                if self.gain_change_hangover == 0 {
                    const MAX_BLOCKS_PER_FRAME: i32 = 3;
                    self.gain_change_hangover = MAX_BLOCKS_PER_FRAME;
                } else {
                    variability.gain_change = false;
                }
            }
            self.subtractor.handle_echo_path_change(&variability);
            self.aec_state.handle_echo_path_change(&variability);
            if variability.delay_changed() {
                self.suppression_gain.set_initial_state(true);
            }
        }
        if self.gain_change_hangover > 0 {
            self.gain_change_hangover -= 1;
        }

        let min_delay = self.aec_state.min_direct_path_filter_delay();
        self.render_signal_analyzer
            .update(render, Some(min_delay.max(0) as usize));
        if self.aec_state.transition_triggered() {
            self.subtractor.exit_initial_state();
            self.suppression_gain.set_initial_state(false);
        }

        let scratch = &mut self.scratch;
        self.subtractor.process(
            render,
            capture,
            &self.render_signal_analyzer,
            self.aec_state.saturated_capture(),
            &mut scratch.subtractor_output,
        );
        for ch in 0..capture.channels() {
            let output = &scratch.subtractor_output[ch];
            let use_refined_output =
                !self.use_coarse_filter_output || refined_output_preferred(output);
            signal_transition(
                if self.refined_filter_output_last_selected {
                    &output.e_refined
                } else {
                    &output.e_coarse
                },
                if use_refined_output {
                    &output.e_refined
                } else {
                    &output.e_coarse
                },
                self.refined_filter_output_last_selected == use_refined_output,
                &mut scratch.e[ch],
            );
            self.refined_filter_output_last_selected = use_refined_output;
            windowed_padded_fft(capture.view(0, ch), &mut self.y_old[ch], &mut scratch.y[ch]);
            windowed_padded_fft(&scratch.e[ch], &mut self.e_old[ch], &mut scratch.e_fft[ch]);
            linear_echo_power(
                &scratch.e_fft[ch],
                &scratch.y[ch],
                &mut scratch.s2_linear[ch],
            );
            scratch.y[ch].spectrum(&mut scratch.y2[ch]);
            scratch.e_fft[ch].spectrum(&mut scratch.e2[ch]);
        }

        self.aec_state.update(&AecStateInput {
            external_delay,
            filter_frequency_responses: self.subtractor.filter_frequency_responses(),
            filter_impulse_responses: self.subtractor.filter_impulse_responses(),
            render,
            e2_refined: &scratch.e2,
            y2: &scratch.y2,
            subtractor_output: &scratch.subtractor_output,
        });

        self.cng.compute(
            self.aec_state.saturated_capture(),
            &scratch.y2,
            &mut scratch.comfort_noise,
            &mut scratch.high_band_comfort_noise,
        );

        self.residual_echo_estimator.estimate(
            &self.aec_state,
            render,
            &scratch.s2_linear,
            &scratch.y2,
            self.suppression_gain.is_dominant_nearend(),
            (&mut scratch.r2, &mut scratch.r2_unbounded),
        );
        let usable_linear_estimate = self.aec_state.usable_linear_estimate();
        if usable_linear_estimate {
            for (e2, y2) in scratch.e2.iter_mut().zip(&scratch.y2) {
                for (e2, y2) in e2.iter_mut().zip(y2) {
                    *e2 = e2.min(*y2);
                }
            }
        }
        let mut gain = [0.0; SPECTRUM_LEN];
        let high_bands_gain = self.suppression_gain.gain(
            &GainInput {
                nearend_spectrum: if usable_linear_estimate {
                    &scratch.e2
                } else {
                    &scratch.y2
                },
                echo_spectrum: if usable_linear_estimate {
                    &scratch.s2_linear
                } else {
                    &scratch.r2
                },
                residual_echo_spectrum: &scratch.r2,
                residual_echo_spectrum_unbounded: &scratch.r2_unbounded,
                comfort_noise_spectrum: self.cng.noise_spectrum(),
                render_signal_analyzer: &self.render_signal_analyzer,
                aec_state: &self.aec_state,
                render: render.block(0),
                clock_drift: self.has_clock_drift || variability.clock_drift,
            },
            &mut gain,
        );
        let y_fft = if self.aec_state.use_linear_filter_output() {
            &scratch.e_fft
        } else {
            &scratch.y
        };
        self.suppression_filter.apply_gain(
            &scratch.comfort_noise,
            &scratch.high_band_comfort_noise,
            &gain,
            high_bands_gain,
            y_fft,
            capture,
        );
    }
}

fn refined_output_preferred(output: &SubtractorOutput) -> bool {
    const LOUD: f32 = 60.0 * 60.0 * BLOCK_LEN as f32;
    if output.e2_coarse < 0.9 * output.e2_refined
        && output.y2 > 30.0 * 30.0 * BLOCK_LEN as f32
        && (output.s2_refined > LOUD || output.s2_coarse > LOUD)
    {
        return false;
    }
    !(output.e2_coarse < output.e2_refined && output.y2 < output.e2_refined)
}

fn linear_echo_power(e: &FftData, y: &FftData, s2: &mut Spectrum) {
    for k in 0..SPECTRUM_LEN {
        let re = y.re[k] - e.re[k];
        let im = y.im[k] - e.im[k];
        s2[k] = re * re + im * im;
    }
}

fn signal_transition(
    from: &[f32; BLOCK_LEN],
    to: &[f32; BLOCK_LEN],
    same: bool,
    out: &mut [f32; BLOCK_LEN],
) {
    if same {
        *out = *to;
        return;
    }
    const TRANSITION_SIZE: usize = 30;
    const ONE_BY_TRANSITION_SIZE_PLUS_ONE: f32 = 1.0 / (TRANSITION_SIZE + 1) as f32;
    for k in 0..TRANSITION_SIZE {
        let a = (k + 1) as f32 * ONE_BY_TRANSITION_SIZE_PLUS_ONE;
        out[k] = a * to[k] + (1.0 - a) * from[k];
    }
    out[TRANSITION_SIZE..].copy_from_slice(&to[TRANSITION_SIZE..]);
}

fn windowed_padded_fft(v: &[f32; BLOCK_LEN], v_old: &mut [f32; BLOCK_LEN], spectrum: &mut FftData) {
    fft::sqrt_hanning_padded_fft(v, v_old, spectrum);
    *v_old = *v;
}
