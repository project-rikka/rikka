use crate::common::{SPECTRUM_LEN, Spectrum};
use crate::config::{CoarseFilter, RefinedFilter};
use crate::echo_path_variability::EchoPathVariability;
use crate::fft::FftData;
use crate::filter::subtractor_output::SubtractorOutput;
use crate::render_signal_analyzer::RenderSignalAnalyzer;

const H_ERROR_INITIAL: f32 = 10_000.0;
const POOR_EXCITATION_COUNTER_INITIAL: usize = 1000;

#[derive(Clone, Copy, Debug)]
pub(crate) struct GainInput<'a> {
    pub(crate) render_power: &'a Spectrum,
    pub(crate) analyzer: &'a RenderSignalAnalyzer,
    pub(crate) size_partitions: usize,
    pub(crate) saturated_capture: bool,
}

pub(crate) fn compute_erl(h2: &[Spectrum], erl: &mut Spectrum) {
    *erl = [0.0; SPECTRUM_LEN];
    for h2 in h2 {
        for (erl, value) in erl.iter_mut().zip(h2) {
            *erl += value;
        }
    }
}

fn average(from: f32, to: f32, from_weight: f32) -> f32 {
    from * from_weight + to * (1.0 - from_weight)
}

#[derive(Clone, Copy, Debug)]
struct Transition<T> {
    current: T,
    target: T,
    old_target: T,
    duration_blocks: i32,
    one_by_duration_blocks: f32,
    counter: i32,
}

impl<T: Copy> Transition<T> {
    fn new(config: T, duration_blocks: usize) -> Transition<T> {
        let duration_blocks = duration_blocks as i32;
        Transition {
            current: config,
            target: config,
            old_target: config,
            duration_blocks,
            one_by_duration_blocks: 1.0 / duration_blocks as f32,
            counter: 0,
        }
    }

    fn set(&mut self, config: T, immediate_effect: bool) {
        if immediate_effect {
            self.current = config;
            self.target = config;
            self.old_target = config;
            self.counter = 0;
        } else {
            self.old_target = self.current;
            self.target = config;
            self.counter = self.duration_blocks;
        }
    }

    fn step(&mut self, blend: impl FnOnce(&mut T, &T, &T, f32)) {
        if self.counter > 0 {
            self.counter -= 1;
            if self.counter > 0 {
                let change_factor = self.counter as f32 * self.one_by_duration_blocks;
                blend(
                    &mut self.current,
                    &self.old_target,
                    &self.target,
                    change_factor,
                );
            } else {
                self.current = self.target;
                self.old_target = self.target;
            }
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RefinedFilterUpdateGain {
    config: Transition<RefinedFilter>,
    h_error: Spectrum,
    poor_excitation_counter: usize,
    call_counter: usize,
}

impl RefinedFilterUpdateGain {
    pub(crate) fn new(
        config: RefinedFilter,
        config_change_duration_blocks: usize,
    ) -> RefinedFilterUpdateGain {
        RefinedFilterUpdateGain {
            config: Transition::new(config, config_change_duration_blocks),
            h_error: [H_ERROR_INITIAL; SPECTRUM_LEN],
            poor_excitation_counter: POOR_EXCITATION_COUNTER_INITIAL,
            call_counter: 0,
        }
    }

    pub(crate) fn set_config(&mut self, config: RefinedFilter, immediate_effect: bool) {
        self.config.set(config, immediate_effect);
    }

    pub(crate) fn handle_echo_path_change(&mut self, variability: &EchoPathVariability) {
        if variability.delay_changed() {
            self.h_error = [H_ERROR_INITIAL; SPECTRUM_LEN];
        }
        if !variability.gain_change {
            self.poor_excitation_counter = POOR_EXCITATION_COUNTER_INITIAL;
            self.call_counter = 0;
        }
    }

    pub(crate) fn compute(
        &mut self,
        input: &GainInput<'_>,
        output: &SubtractorOutput,
        erl: &Spectrum,
        disallow_leakage_diverged: bool,
        g: &mut FftData,
    ) {
        let GainInput {
            render_power: x2,
            analyzer,
            size_partitions,
            saturated_capture,
        } = *input;
        self.call_counter += 1;
        self.config.step(|current, old, target, factor| {
            current.leakage_converged =
                average(old.leakage_converged, target.leakage_converged, factor);
            current.leakage_diverged =
                average(old.leakage_diverged, target.leakage_diverged, factor);
            current.error_floor = average(old.error_floor, target.error_floor, factor);
            current.error_ceil = average(old.error_ceil, target.error_ceil, factor);
            current.noise_gate = average(old.noise_gate, target.noise_gate, factor);
        });
        let config = self.config.current;
        if analyzer.poor_signal_excitation() {
            self.poor_excitation_counter = 0;
        }
        self.poor_excitation_counter += 1;
        if self.poor_excitation_counter < size_partitions
            || saturated_capture
            || self.call_counter <= size_partitions
        {
            g.clear();
        } else {
            let mut mu = [0.0; SPECTRUM_LEN];
            for k in 0..SPECTRUM_LEN {
                if x2[k] >= config.noise_gate {
                    mu[k] = self.h_error[k]
                        / (0.5 * self.h_error[k] * x2[k]
                            + size_partitions as f32 * output.e2_refined_spectrum[k]);
                }
            }
            analyzer.mask_regions_around_narrow_bands(&mut mu);
            for k in 0..SPECTRUM_LEN {
                self.h_error[k] -= 0.5 * mu[k] * x2[k] * self.h_error[k];
            }
            for k in 0..SPECTRUM_LEN {
                g.re[k] = mu[k] * output.e_refined_fft.re[k];
                g.im[k] = mu[k] * output.e_refined_fft.im[k];
            }
        }
        for k in 0..SPECTRUM_LEN {
            if output.e2_refined_spectrum[k] <= output.e2_coarse_spectrum[k]
                || disallow_leakage_diverged
            {
                self.h_error[k] += config.leakage_converged * erl[k];
            } else {
                self.h_error[k] += config.leakage_diverged * erl[k];
            }
            self.h_error[k] = self.h_error[k].max(config.error_floor);
            self.h_error[k] = self.h_error[k].min(config.error_ceil);
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CoarseFilterUpdateGain {
    config: Transition<CoarseFilter>,
    poor_signal_excitation_counter: usize,
    call_counter: usize,
}

impl CoarseFilterUpdateGain {
    pub(crate) fn new(
        config: CoarseFilter,
        config_change_duration_blocks: usize,
    ) -> CoarseFilterUpdateGain {
        CoarseFilterUpdateGain {
            config: Transition::new(config, config_change_duration_blocks),
            poor_signal_excitation_counter: 0,
            call_counter: 0,
        }
    }

    pub(crate) fn set_config(&mut self, config: CoarseFilter, immediate_effect: bool) {
        self.config.set(config, immediate_effect);
    }

    pub(crate) fn handle_echo_path_change(&mut self) {
        self.poor_signal_excitation_counter = 0;
        self.call_counter = 0;
    }

    pub(crate) fn compute(&mut self, input: &GainInput<'_>, e_coarse: &FftData, g: &mut FftData) {
        let GainInput {
            render_power: x2,
            analyzer,
            size_partitions,
            saturated_capture,
        } = *input;
        self.call_counter += 1;
        self.config.step(|current, old, target, factor| {
            current.rate = average(old.rate, target.rate, factor);
            current.noise_gate = average(old.noise_gate, target.noise_gate, factor);
        });
        let config = self.config.current;
        if analyzer.poor_signal_excitation() {
            self.poor_signal_excitation_counter = 0;
        }
        self.poor_signal_excitation_counter += 1;
        if self.poor_signal_excitation_counter < size_partitions
            || saturated_capture
            || self.call_counter <= size_partitions
        {
            g.clear();
            return;
        }
        let mut mu = [0.0; SPECTRUM_LEN];
        for k in 0..SPECTRUM_LEN {
            if x2[k] > config.noise_gate {
                mu[k] = config.rate / x2[k];
            }
        }
        analyzer.mask_regions_around_narrow_bands(&mut mu);
        for k in 0..SPECTRUM_LEN {
            g.re[k] = mu[k] * e_coarse.re[k];
            g.im[k] = mu[k] * e_coarse.im[k];
        }
    }
}
