use rikka_apm_audio::{BANDS, Frame};

use crate::common::{BINS, FFT_SIZE, FRAME_SIZE, OVERLAP, Spectrum};
use crate::fft::Fft;
use crate::level::{Level, SuppressionParams};
use crate::noise::NoiseEstimator;
use crate::signal_model::Observation;
use crate::speech::SpeechProbabilityEstimator;
use crate::wiener::{NoiseSpectra, WienerFilter};
use crate::window::{
    apply_window, delay_signal, energy, energy_of_parts, form_extended_frame, overlap_and_add,
};

type Block = [f32; FFT_SIZE];

pub struct NoiseSuppressor {
    params: SuppressionParams,
    num_analyzed_frames: i32,
    fft: Fft,
    channels: Vec<ChannelState>,
    banks: Vec<BankState>,
}

struct ChannelState {
    speech: SpeechProbabilityEstimator,
    wiener: WienerFilter,
    noise: NoiseEstimator,
    prev_analysis_signal_spectrum: Spectrum,
    analyze_memory: [f32; OVERLAP],
    process_analysis_memory: [f32; OVERLAP],
    process_synthesis_memory: [f32; OVERLAP],
    process_delay_memory: [[f32; OVERLAP]; BANDS - 1],
}

struct BankState {
    real: Block,
    imag: Block,
    extended_frame: Block,
    upper_band_gain: f32,
    energy_before_filtering: f32,
    gain_adjustment: f32,
}

impl NoiseSuppressor {
    #[must_use]
    pub fn new(level: Level, channels: usize) -> NoiseSuppressor {
        let params = SuppressionParams::for_level(level);
        let channels = channels.max(1);
        NoiseSuppressor {
            params,
            num_analyzed_frames: -1,
            fft: Fft::new(),
            channels: (0..channels).map(|_| ChannelState::new(params)).collect(),
            banks: (0..channels).map(|_| BankState::new()).collect(),
        }
    }

    pub fn analyze(&mut self, frame: &Frame) {
        for state in &mut self.channels {
            state.noise.prepare_analysis();
        }

        let has_signal = self.channels.iter().enumerate().any(|(channel, state)| {
            energy_of_parts(frame.band(channel, 0), &state.analyze_memory) > 0.0
        });
        if !has_signal {
            return;
        }

        self.num_analyzed_frames = self.num_analyzed_frames.wrapping_add(1).max(0);
        let num_analyzed_frames = self.num_analyzed_frames;

        for (channel, state) in self.channels.iter_mut().enumerate() {
            let mut extended_frame = [0.0; FFT_SIZE];
            form_extended_frame(
                frame.band(channel, 0),
                &mut state.analyze_memory,
                &mut extended_frame,
            );
            apply_window(&mut extended_frame);

            let mut real = [0.0; FFT_SIZE];
            let mut imag = [0.0; FFT_SIZE];
            self.fft.forward(&mut extended_frame, &mut real, &mut imag);
            let signal_spectrum = magnitude_spectrum(&real, &imag);

            let mut signal_energy = 0.0f32;
            for i in 0..BINS {
                signal_energy += real[i] * real[i] + imag[i] * imag[i];
            }
            signal_energy /= BINS as f32;

            let mut signal_spectral_sum = 0.0f32;
            for &magnitude in &signal_spectrum {
                signal_spectral_sum += magnitude;
            }

            state
                .noise
                .pre_update(num_analyzed_frames, &signal_spectrum, signal_spectral_sum);

            let (prior_snr, post_snr) = snr(
                state.wiener.filter(),
                &state.prev_analysis_signal_spectrum,
                &signal_spectrum,
                state.noise.prev_noise(),
                state.noise.noise(),
            );

            state.speech.update(
                num_analyzed_frames,
                &Observation {
                    prior_snr: &prior_snr,
                    post_snr: &post_snr,
                    conservative_noise: state.noise.conservative_noise(),
                    signal_spectrum: &signal_spectrum,
                    signal_spectral_sum,
                    signal_energy,
                },
            );

            state
                .noise
                .post_update(state.speech.probability(), &signal_spectrum);

            state.prev_analysis_signal_spectrum = signal_spectrum;
        }
    }

    pub fn process(&mut self, frame: &mut Frame) {
        let num_analyzed_frames = self.num_analyzed_frames;
        for (channel, (state, bank)) in self.channels.iter_mut().zip(&mut self.banks).enumerate() {
            form_extended_frame(
                frame.band(channel, 0),
                &mut state.process_analysis_memory,
                &mut bank.extended_frame,
            );
            apply_window(&mut bank.extended_frame);
            bank.energy_before_filtering = energy(&bank.extended_frame);

            self.fft
                .forward(&mut bank.extended_frame, &mut bank.real, &mut bank.imag);
            let signal_spectrum = magnitude_spectrum(&bank.real, &bank.imag);

            state.wiener.update(
                num_analyzed_frames,
                &NoiseSpectra {
                    noise: state.noise.noise(),
                    prev_noise: state.noise.prev_noise(),
                    parametric_noise: state.noise.parametric_noise(),
                },
                &signal_spectrum,
            );

            bank.upper_band_gain = upper_bands_gain(
                self.params.minimum_attenuating_gain,
                state.wiener.filter(),
                state.speech.probability(),
                &state.prev_analysis_signal_spectrum,
                &signal_spectrum,
            );
        }

        let filter = self.aggregated_filter();

        for bank in &mut self.banks {
            for (i, &gain) in filter.iter().enumerate() {
                bank.real[i] *= gain;
                bank.imag[i] *= gain;
            }
        }

        for bank in &mut self.banks {
            self.fft
                .inverse(&bank.real, &bank.imag, &mut bank.extended_frame);
        }

        for (state, bank) in self.channels.iter().zip(&mut self.banks) {
            let energy_after_filtering = energy(&bank.extended_frame);
            apply_window(&mut bank.extended_frame);
            bank.gain_adjustment = state.wiener.overall_scaling_factor(
                num_analyzed_frames,
                state.speech.prior_probability(),
                bank.energy_before_filtering,
                energy_after_filtering,
            );
        }

        let gain_adjustment = self.banks[1..]
            .iter()
            .fold(self.banks[0].gain_adjustment, |gain, bank| {
                gain.min(bank.gain_adjustment)
            });
        for bank in &mut self.banks {
            for sample in &mut bank.extended_frame {
                *sample *= gain_adjustment;
            }
        }

        for (channel, (state, bank)) in self.channels.iter_mut().zip(&self.banks).enumerate() {
            overlap_and_add(
                &bank.extended_frame,
                &mut state.process_synthesis_memory,
                frame.band_mut(channel, 0),
            );
        }

        let upper_band_gain = self.banks[1..]
            .iter()
            .fold(self.banks[0].upper_band_gain, |gain, bank| {
                gain.min(bank.upper_band_gain)
            });
        for (channel, state) in self.channels.iter_mut().enumerate() {
            for band in 1..BANDS {
                let samples = frame.band_mut(channel, band);
                let mut delayed = [0.0; FRAME_SIZE];
                delay_signal(
                    samples,
                    &mut state.process_delay_memory[band - 1],
                    &mut delayed,
                );
                for (sample, &value) in samples.iter_mut().zip(&delayed) {
                    *sample = upper_band_gain * value;
                }
            }
        }

        for channel in 0..self.channels.len() {
            for samples in frame.bands_mut(channel) {
                for sample in samples {
                    *sample = sample.clamp(-32768.0, 32767.0);
                }
            }
        }
    }

    fn aggregated_filter(&self) -> Spectrum {
        let mut filter = *self.channels[0].wiener.filter();
        for state in &self.channels[1..] {
            for (aggregate, &gain) in filter.iter_mut().zip(state.wiener.filter()) {
                *aggregate = aggregate.min(gain);
            }
        }
        filter
    }
}

impl std::fmt::Debug for NoiseSuppressor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NoiseSuppressor")
            .field("channels", &self.channels.len())
            .field("num_analyzed_frames", &self.num_analyzed_frames)
            .finish_non_exhaustive()
    }
}

impl ChannelState {
    fn new(params: SuppressionParams) -> ChannelState {
        ChannelState {
            speech: SpeechProbabilityEstimator::new(),
            wiener: WienerFilter::new(params),
            noise: NoiseEstimator::new(params),
            prev_analysis_signal_spectrum: [1.0; BINS],
            analyze_memory: [0.0; OVERLAP],
            process_analysis_memory: [0.0; OVERLAP],
            process_synthesis_memory: [0.0; OVERLAP],
            process_delay_memory: [[0.0; OVERLAP]; BANDS - 1],
        }
    }
}

impl BankState {
    fn new() -> BankState {
        BankState {
            real: [0.0; FFT_SIZE],
            imag: [0.0; FFT_SIZE],
            extended_frame: [0.0; FFT_SIZE],
            upper_band_gain: 0.0,
            energy_before_filtering: 0.0,
            gain_adjustment: 0.0,
        }
    }
}

fn magnitude_spectrum(real: &Block, imag: &Block) -> Spectrum {
    let mut spectrum = [0.0; BINS];
    spectrum[0] = real[0].abs() + 1.0;
    spectrum[BINS - 1] = real[BINS - 1].abs() + 1.0;
    for i in 1..BINS - 1 {
        spectrum[i] = (real[i] * real[i] + imag[i] * imag[i]).sqrt() + 1.0;
    }
    spectrum
}

fn snr(
    filter: &Spectrum,
    prev_signal_spectrum: &Spectrum,
    signal_spectrum: &Spectrum,
    prev_noise_spectrum: &Spectrum,
    noise_spectrum: &Spectrum,
) -> (Spectrum, Spectrum) {
    let mut prior_snr = [0.0; BINS];
    let mut post_snr = [0.0; BINS];
    for i in 0..BINS {
        let prev_estimate = prev_signal_spectrum[i] / (prev_noise_spectrum[i] + 0.0001) * filter[i];
        post_snr[i] = if signal_spectrum[i] > noise_spectrum[i] {
            signal_spectrum[i] / (noise_spectrum[i] + 0.0001) - 1.0
        } else {
            0.0
        };
        prior_snr[i] = 0.98 * prev_estimate + (1.0 - 0.98) * post_snr[i];
    }
    (prior_snr, post_snr)
}

fn upper_bands_gain(
    minimum_attenuating_gain: f32,
    filter: &Spectrum,
    speech_probability: &Spectrum,
    prev_analysis_signal_spectrum: &Spectrum,
    signal_spectrum: &Spectrum,
) -> f32 {
    const AVERAGED_BINS: usize = 32;
    const ONE_BY_AVERAGED_BINS: f32 = 1.0 / AVERAGED_BINS as f32;

    let mut avg_prob_speech = 0.0f32;
    let mut avg_filter_gain = 0.0f32;
    for i in BINS - AVERAGED_BINS - 1..BINS - 1 {
        avg_prob_speech += speech_probability[i];
        avg_filter_gain += filter[i];
    }
    avg_prob_speech *= ONE_BY_AVERAGED_BINS;
    avg_filter_gain *= ONE_BY_AVERAGED_BINS;

    let mut sum_analysis_spectrum = 0.0f32;
    let mut sum_processing_spectrum = 0.0f32;
    for i in 0..BINS {
        sum_analysis_spectrum += prev_analysis_signal_spectrum[i];
        sum_processing_spectrum += signal_spectrum[i];
    }
    avg_prob_speech *= sum_processing_spectrum / sum_analysis_spectrum;

    let mut gain = 0.5 * (1.0 + (2.0 * avg_prob_speech - 1.0).tanh());
    gain = if avg_prob_speech >= 0.5 {
        0.25 * gain + 0.75 * avg_filter_gain
    } else {
        0.5 * gain + 0.5 * avg_filter_gain
    };
    gain.max(minimum_attenuating_gain).min(1.0)
}
