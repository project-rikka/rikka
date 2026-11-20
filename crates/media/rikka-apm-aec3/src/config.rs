use crate::common::BLOCK_LEN;

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub buffering: Buffering,
    pub delay: Delay,
    pub filter: Filter,
    pub erle: Erle,
    pub ep_strength: EpStrength,
    pub echo_audibility: EchoAudibility,
    pub render_levels: RenderLevels,
    pub echo_removal_control: EchoRemovalControl,
    pub echo_model: EchoModel,
    pub comfort_noise: ComfortNoise,
    pub suppressor: Suppressor,
    pub multi_channel: MultiChannel,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Buffering {
    pub excess_render_detection_interval_blocks: usize,
    pub max_allowed_excess_render_blocks: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Delay {
    pub default_delay: usize,
    pub down_sampling_factor: usize,
    pub num_filters: usize,
    pub delay_headroom_samples: usize,
    pub hysteresis_limit_blocks: usize,
    pub fixed_capture_delay_samples: usize,
    pub delay_estimate_smoothing: f32,
    pub delay_estimate_smoothing_delay_found: f32,
    pub delay_candidate_detection_threshold: f32,
    pub delay_selection_thresholds: DelaySelectionThresholds,
    pub use_external_delay_estimator: bool,
    pub log_warning_on_delay_changes: bool,
    pub render_alignment_mixing: AlignmentMixing,
    pub capture_alignment_mixing: AlignmentMixing,
    pub detect_pre_echo: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DelaySelectionThresholds {
    pub initial: i32,
    pub converged: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AlignmentMixing {
    pub downmix: bool,
    pub adaptive_selection: bool,
    pub activity_power_threshold: f32,
    pub prefer_first_two_channels: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Filter {
    pub refined: RefinedFilter,
    pub coarse: CoarseFilter,
    pub refined_initial: RefinedFilter,
    pub coarse_initial: CoarseFilter,
    pub config_change_duration_blocks: usize,
    pub initial_state_seconds: f32,
    pub coarse_reset_hangover_blocks: i32,
    pub conservative_initial_phase: bool,
    pub enable_coarse_filter_output_usage: bool,
    pub use_linear_filter: bool,
    pub high_pass_filter_echo_reference: bool,
    pub export_linear_aec_output: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RefinedFilter {
    pub length_blocks: usize,
    pub leakage_converged: f32,
    pub leakage_diverged: f32,
    pub error_floor: f32,
    pub error_ceil: f32,
    pub noise_gate: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoarseFilter {
    pub length_blocks: usize,
    pub rate: f32,
    pub noise_gate: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Erle {
    pub min: f32,
    pub max_l: f32,
    pub max_h: f32,
    pub onset_detection: bool,
    pub num_sections: usize,
    pub clamp_quality_estimate_to_zero: bool,
    pub clamp_quality_estimate_to_one: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EpStrength {
    pub default_gain: f32,
    pub default_len: f32,
    pub nearend_len: f32,
    pub echo_can_saturate: bool,
    pub bounded_erl: bool,
    pub erle_onset_compensation_in_dominant_nearend: bool,
    pub use_conservative_tail_frequency_response: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EchoAudibility {
    pub low_render_limit: f32,
    pub normal_render_limit: f32,
    pub floor_power: f32,
    pub audibility_threshold_lf: f32,
    pub audibility_threshold_mf: f32,
    pub audibility_threshold_hf: f32,
    pub use_stationarity_properties: bool,
    pub use_stationarity_properties_at_init: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderLevels {
    pub active_render_limit: f32,
    pub poor_excitation_render_limit: f32,
    pub poor_excitation_render_limit_ds8: f32,
    pub render_power_gain_db: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EchoRemovalControl {
    pub has_clock_drift: bool,
    pub linear_and_stable_echo_path: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EchoModel {
    pub noise_floor_hold: usize,
    pub min_noise_floor_power: f32,
    pub stationary_gate_slope: f32,
    pub noise_gate_power: f32,
    pub noise_gate_slope: f32,
    pub render_pre_window_size: usize,
    pub render_post_window_size: usize,
    pub model_reverb_in_nonlinear_mode: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComfortNoise {
    pub noise_floor_dbfs: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Suppressor {
    pub nearend_average_blocks: usize,
    pub normal_tuning: Tuning,
    pub nearend_tuning: Tuning,
    pub lf_smoothing_during_initial_phase: bool,
    pub last_permanent_lf_smoothing_band: i32,
    pub last_lf_smoothing_band: i32,
    pub last_lf_band: i32,
    pub first_hf_band: i32,
    pub dominant_nearend_detection: DominantNearendDetection,
    pub subband_nearend_detection: SubbandNearendDetection,
    pub use_subband_nearend_detection: bool,
    pub high_bands_suppression: HighBandsSuppression,
    pub floor_first_increase: f32,
    pub conservative_hf_suppression: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaskingThresholds {
    pub enr_transparent: f32,
    pub enr_suppress: f32,
    pub emr_transparent: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tuning {
    pub mask_lf: MaskingThresholds,
    pub mask_hf: MaskingThresholds,
    pub max_inc_factor: f32,
    pub max_dec_factor_lf: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DominantNearendDetection {
    pub enr_threshold: f32,
    pub enr_exit_threshold: f32,
    pub snr_threshold: f32,
    pub hold_duration: i32,
    pub trigger_threshold: i32,
    pub use_during_initial_phase: bool,
    pub use_unbounded_echo_spectrum: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SubbandNearendDetection {
    pub nearend_average_blocks: usize,
    pub subband1: SubbandRegion,
    pub subband2: SubbandRegion,
    pub nearend_threshold: f32,
    pub snr_threshold: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SubbandRegion {
    pub low: usize,
    pub high: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HighBandsSuppression {
    pub enr_threshold: f32,
    pub max_gain_during_echo: f32,
    pub anti_howling_activation_threshold: f32,
    pub anti_howling_gain: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MultiChannel {
    pub detect_stereo_content: bool,
    pub stereo_detection_threshold: f32,
    pub stereo_detection_timeout_threshold_seconds: i32,
    pub stereo_detection_hysteresis_seconds: f32,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            buffering: Buffering {
                excess_render_detection_interval_blocks: 250,
                max_allowed_excess_render_blocks: 8,
            },
            delay: Delay {
                default_delay: 5,
                down_sampling_factor: 4,
                num_filters: 5,
                delay_headroom_samples: 32,
                hysteresis_limit_blocks: 1,
                fixed_capture_delay_samples: 0,
                delay_estimate_smoothing: 0.7,
                delay_estimate_smoothing_delay_found: 0.7,
                delay_candidate_detection_threshold: 0.2,
                delay_selection_thresholds: DelaySelectionThresholds {
                    initial: 5,
                    converged: 20,
                },
                use_external_delay_estimator: false,
                log_warning_on_delay_changes: false,
                render_alignment_mixing: AlignmentMixing {
                    downmix: false,
                    adaptive_selection: true,
                    activity_power_threshold: 10_000.0,
                    prefer_first_two_channels: true,
                },
                capture_alignment_mixing: AlignmentMixing {
                    downmix: false,
                    adaptive_selection: true,
                    activity_power_threshold: 10_000.0,
                    prefer_first_two_channels: false,
                },
                detect_pre_echo: true,
            },
            filter: Filter {
                refined: RefinedFilter {
                    length_blocks: 13,
                    leakage_converged: 0.000_05,
                    leakage_diverged: 0.05,
                    error_floor: 0.001,
                    error_ceil: 2.0,
                    noise_gate: 20_075_344.0,
                },
                coarse: CoarseFilter {
                    length_blocks: 13,
                    rate: 0.7,
                    noise_gate: 20_075_344.0,
                },
                refined_initial: RefinedFilter {
                    length_blocks: 12,
                    leakage_converged: 0.005,
                    leakage_diverged: 0.5,
                    error_floor: 0.001,
                    error_ceil: 2.0,
                    noise_gate: 20_075_344.0,
                },
                coarse_initial: CoarseFilter {
                    length_blocks: 12,
                    rate: 0.9,
                    noise_gate: 20_075_344.0,
                },
                config_change_duration_blocks: 250,
                initial_state_seconds: 2.5,
                coarse_reset_hangover_blocks: 25,
                conservative_initial_phase: false,
                enable_coarse_filter_output_usage: true,
                use_linear_filter: true,
                high_pass_filter_echo_reference: false,
                export_linear_aec_output: false,
            },
            erle: Erle {
                min: 1.0,
                max_l: 4.0,
                max_h: 1.5,
                onset_detection: true,
                num_sections: 1,
                clamp_quality_estimate_to_zero: true,
                clamp_quality_estimate_to_one: true,
            },
            ep_strength: EpStrength {
                default_gain: 1.0,
                default_len: 0.83,
                nearend_len: 0.83,
                echo_can_saturate: true,
                bounded_erl: false,
                erle_onset_compensation_in_dominant_nearend: false,
                use_conservative_tail_frequency_response: true,
            },
            echo_audibility: EchoAudibility {
                low_render_limit: 4.0 * 64.0,
                normal_render_limit: 64.0,
                floor_power: 2.0 * 64.0,
                audibility_threshold_lf: 10.0,
                audibility_threshold_mf: 10.0,
                audibility_threshold_hf: 10.0,
                use_stationarity_properties: false,
                use_stationarity_properties_at_init: false,
            },
            render_levels: RenderLevels {
                active_render_limit: 100.0,
                poor_excitation_render_limit: 150.0,
                poor_excitation_render_limit_ds8: 20.0,
                render_power_gain_db: 0.0,
            },
            echo_removal_control: EchoRemovalControl {
                has_clock_drift: false,
                linear_and_stable_echo_path: false,
            },
            echo_model: EchoModel {
                noise_floor_hold: 50,
                min_noise_floor_power: 1_638_400.0,
                stationary_gate_slope: 10.0,
                noise_gate_power: 27_509.42,
                noise_gate_slope: 0.3,
                render_pre_window_size: 1,
                render_post_window_size: 1,
                model_reverb_in_nonlinear_mode: true,
            },
            comfort_noise: ComfortNoise {
                noise_floor_dbfs: -96.034_06,
            },
            suppressor: Suppressor {
                nearend_average_blocks: 4,
                normal_tuning: Tuning {
                    mask_lf: MaskingThresholds {
                        enr_transparent: 0.3,
                        enr_suppress: 0.4,
                        emr_transparent: 0.3,
                    },
                    mask_hf: MaskingThresholds {
                        enr_transparent: 0.07,
                        enr_suppress: 0.1,
                        emr_transparent: 0.3,
                    },
                    max_inc_factor: 2.0,
                    max_dec_factor_lf: 0.25,
                },
                nearend_tuning: Tuning {
                    mask_lf: MaskingThresholds {
                        enr_transparent: 1.09,
                        enr_suppress: 1.1,
                        emr_transparent: 0.3,
                    },
                    mask_hf: MaskingThresholds {
                        enr_transparent: 0.1,
                        enr_suppress: 0.3,
                        emr_transparent: 0.3,
                    },
                    max_inc_factor: 2.0,
                    max_dec_factor_lf: 0.25,
                },
                lf_smoothing_during_initial_phase: true,
                last_permanent_lf_smoothing_band: 0,
                last_lf_smoothing_band: 5,
                last_lf_band: 5,
                first_hf_band: 8,
                dominant_nearend_detection: DominantNearendDetection {
                    enr_threshold: 0.25,
                    enr_exit_threshold: 10.0,
                    snr_threshold: 30.0,
                    hold_duration: 50,
                    trigger_threshold: 12,
                    use_during_initial_phase: true,
                    use_unbounded_echo_spectrum: true,
                },
                subband_nearend_detection: SubbandNearendDetection {
                    nearend_average_blocks: 1,
                    subband1: SubbandRegion { low: 1, high: 1 },
                    subband2: SubbandRegion { low: 1, high: 1 },
                    nearend_threshold: 1.0,
                    snr_threshold: 1.0,
                },
                use_subband_nearend_detection: false,
                high_bands_suppression: HighBandsSuppression {
                    enr_threshold: 1.0,
                    max_gain_during_echo: 1.0,
                    anti_howling_activation_threshold: 400.0,
                    anti_howling_gain: 1.0,
                },
                floor_first_increase: 0.000_01,
                conservative_hf_suppression: false,
            },
            multi_channel: MultiChannel {
                detect_stereo_content: true,
                stereo_detection_threshold: 0.0,
                stereo_detection_timeout_threshold_seconds: 300,
                stereo_detection_hysteresis_seconds: 2.0,
            },
        }
    }
}

impl Config {
    #[must_use]
    pub fn multichannel() -> Config {
        let mut config = Config::default();
        config.filter.coarse.length_blocks = 11;
        config.filter.coarse.rate = 0.95;
        config.filter.coarse_initial.length_blocks = 11;
        config.filter.coarse_initial.rate = 0.95;
        config.suppressor.normal_tuning.max_dec_factor_lf = 0.35;
        config.suppressor.normal_tuning.max_inc_factor = 1.5;
        config
    }

    pub fn validate(&mut self) -> bool {
        let mut valid = Limits::default();
        let c = self;

        if c.delay.down_sampling_factor != 4 && c.delay.down_sampling_factor != 8 {
            c.delay.down_sampling_factor = 4;
            valid.fail();
        }
        valid.size(&mut c.delay.default_delay, 0, 5000);
        valid.size(&mut c.delay.num_filters, 0, 5000);
        valid.size(&mut c.delay.delay_headroom_samples, 0, 5000);
        valid.size(&mut c.delay.hysteresis_limit_blocks, 0, 5000);
        valid.size(&mut c.delay.fixed_capture_delay_samples, 0, 5000);
        valid.float(&mut c.delay.delay_estimate_smoothing, 0.0, 1.0);
        valid.float(&mut c.delay.delay_candidate_detection_threshold, 0.0, 1.0);
        valid.int(&mut c.delay.delay_selection_thresholds.initial, 1, 250);
        valid.int(&mut c.delay.delay_selection_thresholds.converged, 1, 250);

        valid.refined(&mut c.filter.refined);
        valid.refined(&mut c.filter.refined_initial);
        if c.filter.refined.length_blocks < c.filter.refined_initial.length_blocks {
            c.filter.refined_initial.length_blocks = c.filter.refined.length_blocks;
            valid.fail();
        }
        valid.coarse(&mut c.filter.coarse);
        valid.coarse(&mut c.filter.coarse_initial);
        if c.filter.coarse.length_blocks < c.filter.coarse_initial.length_blocks {
            c.filter.coarse_initial.length_blocks = c.filter.coarse.length_blocks;
            valid.fail();
        }
        valid.size(&mut c.filter.config_change_duration_blocks, 0, 100_000);
        valid.float(&mut c.filter.initial_state_seconds, 0.0, 100.0);
        valid.int(&mut c.filter.coarse_reset_hangover_blocks, 0, 250_000);

        valid.float(&mut c.erle.min, 1.0, 100_000.0);
        valid.float(&mut c.erle.max_l, 1.0, 100_000.0);
        valid.float(&mut c.erle.max_h, 1.0, 100_000.0);
        if c.erle.min > c.erle.max_l || c.erle.min > c.erle.max_h {
            c.erle.min = c.erle.max_l.min(c.erle.max_h);
            valid.fail();
        }
        valid.size(&mut c.erle.num_sections, 1, c.filter.refined.length_blocks);

        valid.float(&mut c.ep_strength.default_gain, 0.0, 1_000_000.0);
        valid.float(&mut c.ep_strength.default_len, -1.0, 1.0);
        valid.float(&mut c.ep_strength.nearend_len, -1.0, 1.0);

        const FULL_SCALE_POWER: f32 = 32_768.0 * 32_768.0;
        valid.float(
            &mut c.echo_audibility.low_render_limit,
            0.0,
            FULL_SCALE_POWER,
        );
        valid.float(
            &mut c.echo_audibility.normal_render_limit,
            0.0,
            FULL_SCALE_POWER,
        );
        valid.float(&mut c.echo_audibility.floor_power, 0.0, FULL_SCALE_POWER);
        valid.float(
            &mut c.echo_audibility.audibility_threshold_lf,
            0.0,
            FULL_SCALE_POWER,
        );
        valid.float(
            &mut c.echo_audibility.audibility_threshold_mf,
            0.0,
            FULL_SCALE_POWER,
        );
        valid.float(
            &mut c.echo_audibility.audibility_threshold_hf,
            0.0,
            FULL_SCALE_POWER,
        );

        valid.float(
            &mut c.render_levels.active_render_limit,
            0.0,
            FULL_SCALE_POWER,
        );
        valid.float(
            &mut c.render_levels.poor_excitation_render_limit,
            0.0,
            FULL_SCALE_POWER,
        );
        valid.float(
            &mut c.render_levels.poor_excitation_render_limit_ds8,
            0.0,
            FULL_SCALE_POWER,
        );

        valid.size(&mut c.echo_model.noise_floor_hold, 0, 1000);
        valid.float(&mut c.echo_model.min_noise_floor_power, 0.0, 2_000_000.0);
        valid.float(&mut c.echo_model.stationary_gate_slope, 0.0, 1_000_000.0);
        valid.float(&mut c.echo_model.noise_gate_power, 0.0, 1_000_000.0);
        valid.float(&mut c.echo_model.noise_gate_slope, 0.0, 1_000_000.0);
        valid.size(&mut c.echo_model.render_pre_window_size, 0, 100);
        valid.size(&mut c.echo_model.render_post_window_size, 0, 100);

        valid.float(&mut c.comfort_noise.noise_floor_dbfs, -200.0, 0.0);

        valid.size(&mut c.suppressor.nearend_average_blocks, 1, 5000);
        valid.tuning(&mut c.suppressor.normal_tuning);
        valid.tuning(&mut c.suppressor.nearend_tuning);
        valid.int(&mut c.suppressor.last_permanent_lf_smoothing_band, 0, 64);
        valid.int(&mut c.suppressor.last_lf_smoothing_band, 0, 64);
        valid.int(&mut c.suppressor.last_lf_band, 0, 63);
        let first_hf_low = c.suppressor.last_lf_band + 1;
        valid.int(&mut c.suppressor.first_hf_band, first_hf_low, 64);

        let dominant = &mut c.suppressor.dominant_nearend_detection;
        valid.float(&mut dominant.enr_threshold, 0.0, 1_000_000.0);
        valid.float(&mut dominant.snr_threshold, 0.0, 1_000_000.0);
        valid.int(&mut dominant.hold_duration, 0, 10_000);
        valid.int(&mut dominant.trigger_threshold, 0, 10_000);

        let subband = &mut c.suppressor.subband_nearend_detection;
        valid.size(&mut subband.nearend_average_blocks, 1, 1024);
        valid.size(&mut subband.subband1.low, 0, 65);
        let low = subband.subband1.low;
        valid.size(&mut subband.subband1.high, low, 65);
        valid.size(&mut subband.subband2.low, 0, 65);
        let low = subband.subband2.low;
        valid.size(&mut subband.subband2.high, low, 65);
        valid.float(&mut subband.nearend_threshold, 0.0, 1.0e24);
        valid.float(&mut subband.snr_threshold, 0.0, 1.0e24);

        let high_bands = &mut c.suppressor.high_bands_suppression;
        valid.float(&mut high_bands.enr_threshold, 0.0, 1_000_000.0);
        valid.float(&mut high_bands.max_gain_during_echo, 0.0, 1.0);
        valid.float(
            &mut high_bands.anti_howling_activation_threshold,
            0.0,
            FULL_SCALE_POWER,
        );
        valid.float(&mut high_bands.anti_howling_gain, 0.0, 1.0);

        valid.float(&mut c.suppressor.floor_first_increase, 0.0, 1_000_000.0);

        valid.valid
    }

    pub(crate) fn delay_headroom_blocks(&self) -> usize {
        self.delay.delay_headroom_samples / BLOCK_LEN
    }
}

#[derive(Debug)]
struct Limits {
    valid: bool,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits { valid: true }
    }
}

impl Limits {
    fn fail(&mut self) {
        self.valid = false;
    }

    fn float(&mut self, value: &mut f32, low: f32, high: f32) {
        let mut clamped = crate::common::safe_clamp(*value, low, high);
        if !clamped.is_finite() {
            clamped = low;
        }
        self.valid &= *value == clamped;
        *value = clamped;
    }

    fn size(&mut self, value: &mut usize, low: usize, high: usize) {
        let clamped = (*value).max(low).min(high.max(low));
        self.valid &= *value == clamped;
        *value = clamped;
    }

    fn int(&mut self, value: &mut i32, low: i32, high: i32) {
        let clamped = (*value).max(low).min(high.max(low));
        self.valid &= *value == clamped;
        *value = clamped;
    }

    fn refined(&mut self, filter: &mut RefinedFilter) {
        self.size(&mut filter.length_blocks, 1, usize::MAX);
        self.float(&mut filter.leakage_converged, 0.0, 1000.0);
        self.float(&mut filter.leakage_diverged, 0.0, 1000.0);
        self.float(&mut filter.error_floor, 0.0, 1000.0);
        self.float(&mut filter.error_ceil, 0.0, 100_000_000.0);
        self.float(&mut filter.noise_gate, 0.0, 100_000_000.0);
    }

    fn coarse(&mut self, filter: &mut CoarseFilter) {
        self.size(&mut filter.length_blocks, 1, usize::MAX);
        self.float(&mut filter.rate, 0.0, 1.0);
        self.float(&mut filter.noise_gate, 0.0, 100_000_000.0);
    }

    fn tuning(&mut self, tuning: &mut Tuning) {
        for mask in [&mut tuning.mask_lf, &mut tuning.mask_hf] {
            self.float(&mut mask.enr_transparent, 0.0, 100.0);
            self.float(&mut mask.enr_suppress, 0.0, 100.0);
            self.float(&mut mask.emr_transparent, 0.0, 100.0);
        }
        self.float(&mut tuning.max_inc_factor, 0.0, 100.0);
        self.float(&mut tuning.max_dec_factor_lf, 0.0, 100.0);
    }
}
