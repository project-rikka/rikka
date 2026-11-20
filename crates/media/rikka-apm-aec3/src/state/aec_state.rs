use crate::common::{BLOCKS_PER_SECOND, FFT_LEN_BY_2, SPECTRUM_LEN, Spectrum};
use crate::config::Config;
use crate::delay::DelayEstimate;
use crate::echo_path_variability::EchoPathVariability;
use crate::filter::{SubtractorOutput, SubtractorOutputAnalyzer};
use crate::render::RenderBuffer;
use crate::ring::Ring;
use crate::state::echo_audibility::EchoAudibility;
use crate::state::erl_estimator::ErlEstimator;
use crate::state::erle::{ErleEstimator, ErleInput};
use crate::state::filter_analyzer::FilterAnalyzer;
use crate::state::filter_delay::FilterDelay;
use crate::state::filtering_quality::{FilteringQualityAnalyzer, QualityInput};
use crate::state::initial_state::InitialState;
use crate::state::reverb::{ReverbInput, ReverbModel, ReverbModelEstimator};
use crate::state::saturation::SaturationDetector;
use crate::state::transparent_mode::{TransparentMode, TransparentModeInput};

#[derive(Clone, Copy, Debug)]
pub(crate) struct AecStateInput<'a> {
    pub(crate) external_delay: Option<DelayEstimate>,
    pub(crate) filter_frequency_responses: &'a [Vec<Spectrum>],
    pub(crate) filter_impulse_responses: &'a [Vec<f32>],
    pub(crate) render: &'a RenderBuffer,
    pub(crate) e2_refined: &'a [Spectrum],
    pub(crate) y2: &'a [Spectrum],
    pub(crate) subtractor_output: &'a [SubtractorOutput],
}

#[derive(Clone, Debug)]
pub(crate) struct AecState {
    use_linear_filter: bool,
    use_stationarity_properties: bool,
    conservative_initial_phase: bool,
    active_render_limit: f32,
    echo_can_saturate: bool,
    initial_state: InitialState,
    delay_state: FilterDelay,
    transparent_state: Option<TransparentMode>,
    filter_quality_state: FilteringQualityAnalyzer,
    saturation_detector: SaturationDetector,
    erl_estimator: ErlEstimator,
    erle_estimator: ErleEstimator,
    strong_not_saturated_render_blocks: usize,
    blocks_with_active_render: usize,
    capture_signal_saturation: bool,
    filter_analyzer: FilterAnalyzer,
    echo_audibility: EchoAudibility,
    reverb_model_estimator: ReverbModelEstimator,
    avg_render_reverb: ReverbModel,
    subtractor_output_analyzer: SubtractorOutputAnalyzer,
}

impl AecState {
    pub(crate) fn new(config: &Config, capture_channels: usize) -> AecState {
        AecState {
            use_linear_filter: config.filter.use_linear_filter,
            use_stationarity_properties: config.echo_audibility.use_stationarity_properties,
            conservative_initial_phase: config.filter.conservative_initial_phase,
            active_render_limit: config.render_levels.active_render_limit,
            echo_can_saturate: config.ep_strength.echo_can_saturate,
            initial_state: InitialState::new(config),
            delay_state: FilterDelay::new(config, capture_channels),
            transparent_state: TransparentMode::create(config),
            filter_quality_state: FilteringQualityAnalyzer::new(
                config.filter.use_linear_filter,
                capture_channels,
            ),
            saturation_detector: SaturationDetector::default(),
            erl_estimator: ErlEstimator::new(2 * BLOCKS_PER_SECOND),
            erle_estimator: ErleEstimator::new(2 * BLOCKS_PER_SECOND, config, capture_channels),
            strong_not_saturated_render_blocks: 0,
            blocks_with_active_render: 0,
            capture_signal_saturation: false,
            filter_analyzer: FilterAnalyzer::new(config, capture_channels),
            echo_audibility: EchoAudibility::new(
                config.echo_audibility.use_stationarity_properties_at_init,
            ),
            reverb_model_estimator: ReverbModelEstimator::new(config, capture_channels),
            avg_render_reverb: ReverbModel::new(),
            subtractor_output_analyzer: SubtractorOutputAnalyzer::new(capture_channels),
        }
    }

    pub(crate) fn usable_linear_estimate(&self) -> bool {
        self.filter_quality_state.linear_filter_usable() && self.use_linear_filter
    }

    pub(crate) fn use_linear_filter_output(&self) -> bool {
        self.usable_linear_estimate()
    }

    pub(crate) fn residual_echo_scaling(&self, residual_scaling: &mut Spectrum) {
        let seconds = if self.conservative_initial_phase {
            1.5
        } else {
            0.8
        };
        let filter_has_had_time_to_converge =
            self.strong_not_saturated_render_blocks as f32 >= seconds * BLOCKS_PER_SECOND as f32;
        self.echo_audibility
            .residual_echo_scaling(filter_has_had_time_to_converge, residual_scaling);
    }

    pub(crate) fn use_stationarity_properties(&self) -> bool {
        self.use_stationarity_properties
    }

    pub(crate) fn erle(&self, onset_compensated: bool) -> &[Spectrum] {
        self.erle_estimator.erle(onset_compensated)
    }

    pub(crate) fn erle_unbounded(&self) -> &[Spectrum] {
        self.erle_estimator.erle_unbounded()
    }

    pub(crate) fn fullband_erle_log2(&self) -> f32 {
        self.erle_estimator.fullband_erle_log2()
    }

    pub(crate) fn erl_time_domain(&self) -> f32 {
        self.erl_estimator.erl_time_domain()
    }

    pub(crate) fn min_direct_path_filter_delay(&self) -> i32 {
        self.delay_state.min_direct_path_filter_delay()
    }

    pub(crate) fn saturated_capture(&self) -> bool {
        self.capture_signal_saturation
    }

    pub(crate) fn saturated_echo(&self) -> bool {
        self.saturation_detector.saturated_echo()
    }

    pub(crate) fn update_capture_saturation(&mut self, saturation: bool) {
        self.capture_signal_saturation = saturation;
    }

    pub(crate) fn transparent_mode_active(&self) -> bool {
        self.transparent_state
            .as_ref()
            .is_some_and(TransparentMode::active)
    }

    pub(crate) fn reverb_decay(&self, mild: bool) -> f32 {
        self.reverb_model_estimator.reverb_decay(mild)
    }

    pub(crate) fn reverb_frequency_response(&self) -> &Spectrum {
        self.reverb_model_estimator.reverb_frequency_response()
    }

    pub(crate) fn transition_triggered(&self) -> bool {
        self.initial_state.transition_triggered()
    }

    pub(crate) fn filter_length_blocks(&self) -> i32 {
        self.filter_analyzer.filter_length_blocks()
    }

    pub(crate) fn handle_echo_path_change(&mut self, variability: &EchoPathVariability) {
        if variability.delay_changed() {
            self.filter_analyzer.reset();
            self.capture_signal_saturation = false;
            self.strong_not_saturated_render_blocks = 0;
            self.blocks_with_active_render = 0;
            self.initial_state.reset();
            if let Some(transparent) = &mut self.transparent_state {
                transparent.reset();
            }
            self.erle_estimator.reset(true);
            self.erl_estimator.reset();
            self.filter_quality_state.reset();
        } else if variability.gain_change {
            self.erle_estimator.reset(false);
        }
        self.subtractor_output_analyzer.handle_echo_path_change();
    }

    pub(crate) fn update(&mut self, input: &AecStateInput<'_>) {
        let render = input.render;
        let convergence = self
            .subtractor_output_analyzer
            .update(input.subtractor_output);
        let analysis = self
            .filter_analyzer
            .update(input.filter_impulse_responses, render);
        if self.use_linear_filter {
            self.delay_state.update(
                self.filter_analyzer.filter_delays_blocks(),
                input.external_delay,
                self.strong_not_saturated_render_blocks,
            );
        }

        let min_delay = self.delay_state.min_direct_path_filter_delay();
        let aligned_render_block = render.block(-(min_delay as isize));
        let threshold = self.active_render_limit * self.active_render_limit * FFT_LEN_BY_2 as f32;
        let active_render = (0..aligned_render_block.channels())
            .any(|ch| crate::common::sum_of_squares(aligned_render_block.view(0, ch)) > threshold);
        self.blocks_with_active_render += usize::from(active_render);
        self.strong_not_saturated_render_blocks +=
            usize::from(active_render && !self.saturated_capture());

        let mut avg_render_spectrum_with_reverb = [0.0; SPECTRUM_LEN];
        let reverb_decay = self.reverb_decay(false);
        compute_avg_render_reverb(
            &render.spectra,
            min_delay,
            reverb_decay,
            &mut self.avg_render_reverb,
            &mut avg_render_spectrum_with_reverb,
        );

        if self.use_stationarity_properties {
            self.echo_audibility.update(
                render,
                self.avg_render_reverb.reverb(),
                min_delay,
                self.delay_state.external_delay_reported(),
            );
        }

        if self.initial_state.transition_triggered() {
            self.erle_estimator.reset(false);
        }
        self.erle_estimator.update(&ErleInput {
            render,
            filter_frequency_responses: input.filter_frequency_responses,
            avg_render_spectrum_with_reverb: &avg_render_spectrum_with_reverb,
            capture_spectra: input.y2,
            subtractor_spectra: input.e2_refined,
            converged_filters: self.subtractor_output_analyzer.converged_filters(),
        });
        self.erl_estimator.update(
            self.subtractor_output_analyzer.converged_filters(),
            render.spectrum(min_delay as isize),
            input.y2,
        );

        if self.echo_can_saturate {
            let usable_linear_estimate = self.usable_linear_estimate();
            self.saturation_detector.update(
                aligned_render_block,
                self.capture_signal_saturation,
                usable_linear_estimate,
                input.subtractor_output,
                analysis.max_echo_path_gain,
            );
        }

        self.initial_state
            .update(active_render, self.capture_signal_saturation);

        if let Some(transparent) = &mut self.transparent_state {
            transparent.update(&TransparentModeInput {
                filter_delay_blocks: min_delay,
                any_filter_consistent: analysis.any_filter_consistent,
                any_filter_converged: convergence.any_filter_converged,
                all_filters_diverged: convergence.all_filters_diverged,
                active_render,
                saturated_capture: self.capture_signal_saturation,
            });
        }

        let transparent_mode = self.transparent_mode_active();
        self.filter_quality_state.update(&QualityInput {
            active_render,
            transparent_mode,
            saturated_capture: self.capture_signal_saturation,
            external_delay: input.external_delay.is_some(),
            any_filter_converged: convergence.any_filter_converged,
        });

        let stationary_block =
            self.use_stationarity_properties && self.echo_audibility.is_block_stationary();
        self.reverb_model_estimator.update(&ReverbInput {
            impulse_responses: self.filter_analyzer.adjusted_filters(),
            frequency_responses: input.filter_frequency_responses,
            linear_filter_qualities: self.erle_estimator.inst_linear_quality_estimates(),
            filter_delays_blocks: self.delay_state.direct_path_filter_delays(),
            usable_linear_estimates: self.filter_quality_state.usable_linear_filter_outputs(),
            stationary_block,
        });
    }
}

fn compute_avg_render_reverb(
    spectra: &Ring<Vec<Spectrum>>,
    delay_blocks: i32,
    reverb_decay: f32,
    reverb_model: &mut ReverbModel,
    reverb_power_spectrum: &mut Spectrum,
) {
    let channels = spectra.buffer[0].len();
    let idx_at_delay = spectra.offset(spectra.read, delay_blocks as isize);
    let idx_past = spectra.inc(idx_at_delay);
    let x2 = if channels > 1 {
        let average = |spectrum: &[Spectrum]| {
            let mut power = [0.0_f32; SPECTRUM_LEN];
            for channel in spectrum {
                for (power, value) in power.iter_mut().zip(channel) {
                    *power += value;
                }
            }
            let normalizer = 1.0 / channels as f32;
            for power in &mut power {
                *power *= normalizer;
            }
            power
        };
        reverb_model.update_without_shaping(&average(&spectra.buffer[idx_past]), 1.0, reverb_decay);
        average(&spectra.buffer[idx_at_delay])
    } else {
        reverb_model.update_without_shaping(&spectra.buffer[idx_past][0], 1.0, reverb_decay);
        spectra.buffer[idx_at_delay][0]
    };
    let reverb_power = reverb_model.reverb();
    for k in 0..SPECTRUM_LEN {
        reverb_power_spectrum[k] = x2[k] + reverb_power[k];
    }
}
