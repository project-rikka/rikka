use crate::common::{SPECTRUM_LEN, Spectrum};
use crate::config::{EchoModel, EpStrength};
use crate::render::RenderBuffer;
use crate::ring::Ring;
use crate::state::{AecState, ReverbModel};

const TRANSPARENT_MODE_GAIN: f32 = 0.01;

#[derive(Clone, Copy, Debug, PartialEq)]
enum ReverbType {
    Linear,
    NonLinear,
}

#[derive(Clone, Debug)]
pub(crate) struct ResidualEchoEstimator {
    echo_model: EchoModel,
    render_channels: usize,
    general_gain: f32,
    erle_onset_compensation_in_dominant_nearend: bool,
    x2_noise_floor: Spectrum,
    x2_noise_floor_counter: [i32; SPECTRUM_LEN],
    echo_reverb: ReverbModel,
}

impl ResidualEchoEstimator {
    pub(crate) fn new(
        echo_model: &EchoModel,
        ep_strength: &EpStrength,
        render_channels: usize,
    ) -> ResidualEchoEstimator {
        ResidualEchoEstimator {
            echo_model: echo_model.clone(),
            render_channels,
            general_gain: ep_strength.default_gain,
            erle_onset_compensation_in_dominant_nearend: ep_strength
                .erle_onset_compensation_in_dominant_nearend,
            x2_noise_floor: [echo_model.min_noise_floor_power; SPECTRUM_LEN],
            x2_noise_floor_counter: [echo_model.noise_floor_hold as i32; SPECTRUM_LEN],
            echo_reverb: ReverbModel::new(),
        }
    }

    pub(crate) fn estimate(
        &mut self,
        aec_state: &AecState,
        render: &RenderBuffer,
        s2_linear: &[Spectrum],
        y2: &[Spectrum],
        dominant_nearend: bool,
        residual: (&mut [Spectrum], &mut [Spectrum]),
    ) {
        let (r2, r2_unbounded) = residual;
        self.update_render_noise_power(render);
        if aec_state.usable_linear_estimate() {
            if aec_state.saturated_echo() {
                r2.copy_from_slice(y2);
                r2_unbounded.copy_from_slice(y2);
            } else {
                let onset_compensated =
                    self.erle_onset_compensation_in_dominant_nearend || !dominant_nearend;
                linear_estimate(s2_linear, aec_state.erle(onset_compensated), r2);
                linear_estimate(s2_linear, aec_state.erle_unbounded(), r2_unbounded);
            }
            self.update_reverb(ReverbType::Linear, aec_state, render, dominant_nearend);
            self.add_reverb(r2);
            self.add_reverb(r2_unbounded);
        } else {
            let echo_path_gain = self.echo_path_gain(aec_state);
            if aec_state.saturated_echo() {
                r2.copy_from_slice(y2);
                r2_unbounded.copy_from_slice(y2);
            } else {
                let mut x2 = [0.0; SPECTRUM_LEN];
                echo_generating_power(
                    self.render_channels,
                    &render.spectra,
                    &self.echo_model,
                    aec_state.min_direct_path_filter_delay(),
                    &mut x2,
                );
                if !aec_state.use_stationarity_properties() {
                    apply_noise_gate(&self.echo_model, &mut x2);
                }
                for (x2, floor) in x2.iter_mut().zip(&self.x2_noise_floor) {
                    *x2 -= self.echo_model.stationary_gate_slope * floor;
                    *x2 = 0.0_f32.max(*x2);
                }
                non_linear_estimate(echo_path_gain, &x2, r2);
                non_linear_estimate(echo_path_gain, &x2, r2_unbounded);
            }
            if self.echo_model.model_reverb_in_nonlinear_mode
                && !aec_state.transparent_mode_active()
            {
                self.update_reverb(ReverbType::NonLinear, aec_state, render, dominant_nearend);
                self.add_reverb(r2);
                self.add_reverb(r2_unbounded);
            }
        }

        if aec_state.use_stationarity_properties() {
            let mut residual_scaling = [0.0; SPECTRUM_LEN];
            aec_state.residual_echo_scaling(&mut residual_scaling);
            for (r2, r2_unbounded) in r2.iter_mut().zip(r2_unbounded.iter_mut()) {
                for k in 0..SPECTRUM_LEN {
                    r2[k] *= residual_scaling[k];
                    r2_unbounded[k] *= residual_scaling[k];
                }
            }
        }
    }

    fn update_render_noise_power(&mut self, render: &RenderBuffer) {
        let render_power = channel_sum(render.spectrum(0), self.render_channels);
        let hold = self.echo_model.noise_floor_hold as i32;
        for k in 0..SPECTRUM_LEN {
            if render_power[k] < self.x2_noise_floor[k] {
                self.x2_noise_floor[k] = render_power[k];
                self.x2_noise_floor_counter[k] = 0;
            } else if self.x2_noise_floor_counter[k] >= hold {
                self.x2_noise_floor[k] =
                    (self.x2_noise_floor[k] * 1.1).max(self.echo_model.min_noise_floor_power);
            } else {
                self.x2_noise_floor_counter[k] += 1;
            }
        }
    }

    fn update_reverb(
        &mut self,
        reverb_type: ReverbType,
        aec_state: &AecState,
        render: &RenderBuffer,
        dominant_nearend: bool,
    ) {
        let first_reverb_partition = match reverb_type {
            ReverbType::Linear => aec_state.filter_length_blocks() + 1,
            ReverbType::NonLinear => aec_state.min_direct_path_filter_delay() + 1,
        };
        let render_power = channel_sum(
            render.spectrum(first_reverb_partition as isize),
            self.render_channels,
        );
        let reverb_decay = aec_state.reverb_decay(dominant_nearend);
        match reverb_type {
            ReverbType::Linear => {
                self.echo_reverb.update(
                    &render_power,
                    aec_state.reverb_frequency_response(),
                    reverb_decay,
                );
            }
            ReverbType::NonLinear => {
                let echo_path_gain = self.echo_path_gain(aec_state);
                self.echo_reverb.update_without_shaping(
                    &render_power,
                    echo_path_gain,
                    reverb_decay,
                );
            }
        }
    }

    fn add_reverb(&self, r2: &mut [Spectrum]) {
        let reverb_power = self.echo_reverb.reverb();
        for r2 in r2 {
            for (value, reverb) in r2.iter_mut().zip(reverb_power) {
                *value += reverb;
            }
        }
    }

    fn echo_path_gain(&self, aec_state: &AecState) -> f32 {
        let gain_amplitude = if aec_state.transparent_mode_active() {
            TRANSPARENT_MODE_GAIN
        } else {
            self.general_gain
        };
        gain_amplitude * gain_amplitude
    }
}

fn channel_sum(spectra: &[Spectrum], channels: usize) -> Spectrum {
    if channels == 1 {
        return spectra[0];
    }
    let mut sum = [0.0_f32; SPECTRUM_LEN];
    for spectrum in &spectra[..channels] {
        for (sum, value) in sum.iter_mut().zip(spectrum) {
            *sum += value;
        }
    }
    sum
}

fn linear_estimate(s2_linear: &[Spectrum], erle: &[Spectrum], r2: &mut [Spectrum]) {
    for ((r2, s2), erle) in r2.iter_mut().zip(s2_linear).zip(erle) {
        for k in 0..SPECTRUM_LEN {
            r2[k] = s2[k] / erle[k];
        }
    }
}

fn non_linear_estimate(echo_path_gain: f32, x2: &Spectrum, r2: &mut [Spectrum]) {
    for r2 in r2 {
        for (value, x2) in r2.iter_mut().zip(x2) {
            *value = x2 * echo_path_gain;
        }
    }
}

fn apply_noise_gate(config: &EchoModel, x2: &mut Spectrum) {
    for value in x2 {
        if config.noise_gate_power > *value {
            *value =
                0.0_f32.max(*value - config.noise_gate_slope * (config.noise_gate_power - *value));
        }
    }
}

fn echo_generating_power(
    render_channels: usize,
    spectra: &Ring<Vec<Spectrum>>,
    echo_model: &EchoModel,
    filter_delay_blocks: i32,
    x2: &mut Spectrum,
) {
    let window_start = (filter_delay_blocks - echo_model.render_pre_window_size as i32).max(0);
    let window_end = filter_delay_blocks + echo_model.render_post_window_size as i32;
    let idx_start = spectra.offset(spectra.read, window_start as isize);
    let idx_stop = spectra.offset(spectra.read, window_end as isize + 1);
    *x2 = [0.0; SPECTRUM_LEN];
    let mut k = idx_start;
    while k != idx_stop {
        let power = channel_sum(&spectra.buffer[k], render_channels);
        for (max, value) in x2.iter_mut().zip(&power) {
            *max = max.max(*value);
        }
        k = spectra.inc(k);
    }
}
