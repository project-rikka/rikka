use crate::block::Block;
use crate::common::{BLOCK_LEN, FFT_LEN, FFT_LEN_BY_2, SPECTRUM_LEN, Spectrum, time_domain_len};
use crate::config::Config;
use crate::echo_path_variability::EchoPathVariability;
use crate::fft::{self, FftData};
use crate::filter::adaptive_fir_filter::AdaptiveFirFilter;
use crate::filter::subtractor_output::SubtractorOutput;
use crate::filter::update_gain::{
    CoarseFilterUpdateGain, GainInput, RefinedFilterUpdateGain, compute_erl,
};
use crate::render::RenderBuffer;
use crate::render_signal_analyzer::RenderSignalAnalyzer;

#[derive(Clone, Debug)]
struct Channel {
    refined_filter: AdaptiveFirFilter,
    coarse_filter: AdaptiveFirFilter,
    refined_gain: RefinedFilterUpdateGain,
    coarse_gain: CoarseFilterUpdateGain,
    misadjustment: FilterMisadjustmentEstimator,
    poor_coarse_filter_counter: usize,
    coarse_filter_reset_hangover: i32,
}

#[derive(Clone, Debug)]
pub(crate) struct Subtractor {
    config: Config,
    channels: Vec<Channel>,
    refined_frequency_responses: Vec<Vec<Spectrum>>,
    refined_impulse_responses: Vec<Vec<f32>>,
}

impl Subtractor {
    pub(crate) fn new(
        config: &Config,
        render_channels: usize,
        capture_channels: usize,
    ) -> Subtractor {
        let filter = &config.filter;
        let refined_max_blocks = filter
            .refined_initial
            .length_blocks
            .max(filter.refined.length_blocks);
        let channel = Channel {
            refined_filter: AdaptiveFirFilter::new(
                filter.refined.length_blocks,
                filter.refined_initial.length_blocks,
                filter.config_change_duration_blocks,
                render_channels,
            ),
            coarse_filter: AdaptiveFirFilter::new(
                filter.coarse.length_blocks,
                filter.coarse_initial.length_blocks,
                filter.config_change_duration_blocks,
                render_channels,
            ),
            refined_gain: RefinedFilterUpdateGain::new(
                filter.refined_initial,
                filter.config_change_duration_blocks,
            ),
            coarse_gain: CoarseFilterUpdateGain::new(
                filter.coarse_initial,
                filter.config_change_duration_blocks,
            ),
            misadjustment: FilterMisadjustmentEstimator::default(),
            poor_coarse_filter_counter: 0,
            coarse_filter_reset_hangover: 0,
        };
        Subtractor {
            config: config.clone(),
            channels: vec![channel; capture_channels],
            refined_frequency_responses: vec![
                vec![[0.0; SPECTRUM_LEN]; refined_max_blocks];
                capture_channels
            ],
            refined_impulse_responses: vec![
                vec![0.0; time_domain_len(refined_max_blocks)];
                capture_channels
            ],
        }
    }

    pub(crate) fn filter_frequency_responses(&self) -> &[Vec<Spectrum>] {
        &self.refined_frequency_responses
    }

    pub(crate) fn filter_impulse_responses(&self) -> &[Vec<f32>] {
        &self.refined_impulse_responses
    }

    pub(crate) fn handle_echo_path_change(&mut self, variability: &EchoPathVariability) {
        let filter = &self.config.filter;
        if variability.delay_changed() {
            for channel in &mut self.channels {
                channel.refined_filter.handle_echo_path_change();
                channel.coarse_filter.handle_echo_path_change();
                channel.refined_gain.handle_echo_path_change(variability);
                channel.coarse_gain.handle_echo_path_change();
                channel
                    .refined_gain
                    .set_config(filter.refined_initial, true);
                channel.coarse_gain.set_config(filter.coarse_initial, true);
                channel
                    .refined_filter
                    .set_size_partitions(filter.refined_initial.length_blocks, true);
                channel
                    .coarse_filter
                    .set_size_partitions(filter.coarse_initial.length_blocks, true);
            }
        }
        if variability.gain_change {
            for channel in &mut self.channels {
                channel.refined_gain.handle_echo_path_change(variability);
            }
        }
    }

    pub(crate) fn exit_initial_state(&mut self) {
        let filter = &self.config.filter;
        for channel in &mut self.channels {
            channel.refined_gain.set_config(filter.refined, false);
            channel.coarse_gain.set_config(filter.coarse, false);
            channel
                .refined_filter
                .set_size_partitions(filter.refined.length_blocks, false);
            channel
                .coarse_filter
                .set_size_partitions(filter.coarse.length_blocks, false);
        }
    }

    pub(crate) fn process(
        &mut self,
        render: &RenderBuffer,
        capture: &Block,
        analyzer: &RenderSignalAnalyzer,
        saturated_capture: bool,
        outputs: &mut [SubtractorOutput],
    ) {
        let refined_partitions = self.channels[0].refined_filter.size_partitions();
        let coarse_partitions = self.channels[0].coarse_filter.size_partitions();
        let mut x2_refined = [0.0; SPECTRUM_LEN];
        let mut x2_coarse = [0.0; SPECTRUM_LEN];
        if refined_partitions == coarse_partitions {
            render.spectral_sum(refined_partitions, &mut x2_refined);
            x2_coarse = x2_refined;
        } else if refined_partitions > coarse_partitions {
            render.spectral_sums(
                coarse_partitions,
                refined_partitions,
                &mut x2_coarse,
                &mut x2_refined,
            );
        } else {
            render.spectral_sums(
                refined_partitions,
                coarse_partitions,
                &mut x2_refined,
                &mut x2_coarse,
            );
        }

        for (ch, ((channel, output), (frequency_response, impulse_response))) in self
            .channels
            .iter_mut()
            .zip(outputs.iter_mut())
            .zip(
                self.refined_frequency_responses
                    .iter_mut()
                    .zip(self.refined_impulse_responses.iter_mut()),
            )
            .enumerate()
        {
            let y = capture.view(0, ch);
            let mut s = FftData::default();
            channel.refined_filter.filter(render, &mut s);
            prediction_error(&s, y, &mut output.e_refined, &mut output.s_refined);
            channel.coarse_filter.filter(render, &mut s);
            prediction_error(&s, y, &mut output.e_coarse, &mut output.s_coarse);
            output.compute_metrics(y);

            let mut refined_filters_adjusted = false;
            channel.misadjustment.update(output);
            if channel.misadjustment.is_adjustment_needed() {
                let scale = channel.misadjustment.misadjustment();
                channel.refined_filter.scale_filter(scale);
                for h in impulse_response.iter_mut() {
                    *h *= scale;
                }
                for ((s, e), y) in output
                    .s_refined
                    .iter_mut()
                    .zip(output.e_refined.iter_mut())
                    .zip(y)
                {
                    *s *= scale;
                    *e = y - *s;
                }
                channel.misadjustment.reset();
                refined_filters_adjusted = true;
            }

            let mut e_coarse_fft = FftData::default();
            fft::zero_padded_hanning_fft(&output.e_refined, &mut output.e_refined_fft);
            fft::zero_padded_hanning_fft(&output.e_coarse, &mut e_coarse_fft);
            e_coarse_fft.spectrum(&mut output.e2_coarse_spectrum);
            output
                .e_refined_fft
                .spectrum(&mut output.e2_refined_spectrum);

            let g = &mut s;
            if refined_filters_adjusted {
                g.clear();
            } else {
                let disallow_leakage_diverged = channel.coarse_filter_reset_hangover > 0;
                let mut erl = [0.0; SPECTRUM_LEN];
                compute_erl(frequency_response, &mut erl);
                let input = GainInput {
                    render_power: &x2_refined,
                    analyzer,
                    size_partitions: channel.refined_filter.size_partitions(),
                    saturated_capture,
                };
                channel
                    .refined_gain
                    .compute(&input, output, &erl, disallow_leakage_diverged, g);
            }
            channel
                .refined_filter
                .adapt_with_impulse_response(render, g, impulse_response);
            channel
                .refined_filter
                .compute_frequency_response(frequency_response);

            channel.poor_coarse_filter_counter = if output.e2_refined < output.e2_coarse {
                channel.poor_coarse_filter_counter + 1
            } else {
                0
            };
            let coarse_error = if channel.poor_coarse_filter_counter < 5 {
                channel.coarse_filter_reset_hangover =
                    (channel.coarse_filter_reset_hangover - 1).max(0);
                &e_coarse_fft
            } else {
                channel.poor_coarse_filter_counter = 0;
                channel.coarse_filter.set_filter(
                    channel.refined_filter.size_partitions(),
                    channel.refined_filter.filter_partitions(),
                );
                channel.coarse_filter_reset_hangover =
                    self.config.filter.coarse_reset_hangover_blocks;
                &output.e_refined_fft
            };
            let input = GainInput {
                render_power: &x2_coarse,
                analyzer,
                size_partitions: channel.coarse_filter.size_partitions(),
                saturated_capture,
            };
            channel.coarse_gain.compute(&input, coarse_error, g);
            channel.coarse_filter.adapt(render, g);

            for e in &mut output.e_refined {
                *e = crate::common::safe_clamp(*e, -32_768.0, 32_767.0);
            }
        }
    }
}

fn prediction_error(
    s: &FftData,
    y: &[f32; BLOCK_LEN],
    e: &mut [f32; BLOCK_LEN],
    s_out: &mut [f32; BLOCK_LEN],
) {
    const SCALE: f32 = 1.0 / FFT_LEN_BY_2 as f32;
    let mut time = [0.0; FFT_LEN];
    fft::ifft(s, &mut time);
    let estimate = &time[FFT_LEN_BY_2..];
    for ((e, y), value) in e.iter_mut().zip(y).zip(estimate) {
        *e = y - value * SCALE;
    }
    for (s_out, value) in s_out.iter_mut().zip(estimate) {
        *s_out = SCALE * value;
    }
}

#[derive(Clone, Debug, Default)]
struct FilterMisadjustmentEstimator {
    blocks_accumulated: i32,
    e2_accumulated: f32,
    y2_accumulated: f32,
    inv_misadjustment: f32,
    overhang: i32,
}

impl FilterMisadjustmentEstimator {
    const BLOCKS: i32 = 4;

    fn update(&mut self, output: &SubtractorOutput) {
        self.e2_accumulated += output.e2_refined;
        self.y2_accumulated += output.y2;
        self.blocks_accumulated += 1;
        if self.blocks_accumulated != Self::BLOCKS {
            return;
        }
        if self.y2_accumulated > Self::BLOCKS as f32 * 200.0 * 200.0 * BLOCK_LEN as f32 {
            let update = self.e2_accumulated / self.y2_accumulated;
            if self.e2_accumulated > Self::BLOCKS as f32 * 7500.0 * 7500.0 * BLOCK_LEN as f32 {
                self.overhang = 4;
            } else {
                self.overhang = (self.overhang - 1).max(0);
            }
            if update < self.inv_misadjustment || self.overhang > 0 {
                self.inv_misadjustment += 0.1 * (update - self.inv_misadjustment);
            }
        }
        self.e2_accumulated = 0.0;
        self.y2_accumulated = 0.0;
        self.blocks_accumulated = 0;
    }

    fn misadjustment(&self) -> f32 {
        2.0 / self.inv_misadjustment.sqrt()
    }

    fn is_adjustment_needed(&self) -> bool {
        self.inv_misadjustment > 10.0
    }

    fn reset(&mut self) {
        *self = FilterMisadjustmentEstimator::default();
    }
}
