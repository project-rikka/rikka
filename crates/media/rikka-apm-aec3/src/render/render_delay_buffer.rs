use super::alignment_mixer::AlignmentMixer;
use super::decimator::Decimator;
use super::render_buffer::RenderBuffer;
use crate::block::Block;
use crate::common::{BLOCK_LEN, downsampled_buffer_len, render_delay_buffer_len};
use crate::config::Config;
use crate::fft;
use crate::ring::Ring;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum BufferingEvent {
    None,
    RenderUnderrun,
    RenderOverrun,
}

#[derive(Clone, Debug)]
pub(crate) struct RenderDelayBuffer {
    excess_render_detection_interval_blocks: usize,
    max_allowed_excess_render_blocks: usize,
    delay_headroom_blocks: usize,
    default_delay: usize,
    render_linear_amplitude_gain: f32,
    sub_block_size: usize,
    render: RenderBuffer,
    delay: Option<usize>,
    low_rate: Ring<f32>,
    render_mixer: AlignmentMixer,
    render_decimator: Decimator,
    render_ds: Vec<f32>,
    buffer_headroom: usize,
    capture_call_counter: i64,
    render_call_counter: i64,
    external_audio_buffer_delay: Option<i32>,
    min_latency_blocks: usize,
    excess_render_detection_counter: usize,
}

impl RenderDelayBuffer {
    pub(crate) fn new(config: &Config, bands: usize, render_channels: usize) -> RenderDelayBuffer {
        let down_sampling_factor = config.delay.down_sampling_factor;
        let sub_block_size = BLOCK_LEN
            .checked_div(down_sampling_factor)
            .unwrap_or(BLOCK_LEN);
        let len = render_delay_buffer_len(
            down_sampling_factor,
            config.delay.num_filters,
            config.filter.refined.length_blocks,
        );
        let mut buffer = RenderDelayBuffer {
            excess_render_detection_interval_blocks: config
                .buffering
                .excess_render_detection_interval_blocks,
            max_allowed_excess_render_blocks: config.buffering.max_allowed_excess_render_blocks,
            delay_headroom_blocks: config.delay_headroom_blocks(),
            default_delay: config.delay.default_delay,
            render_linear_amplitude_gain: 10.0_f32
                .powf(config.render_levels.render_power_gain_db / 20.0),
            sub_block_size,
            render: RenderBuffer::new(len, bands, render_channels),
            delay: Some(config.delay.default_delay),
            low_rate: Ring::new(
                downsampled_buffer_len(down_sampling_factor, config.delay.num_filters),
                0.0,
            ),
            render_mixer: AlignmentMixer::new(
                render_channels,
                &config.delay.render_alignment_mixing,
            ),
            render_decimator: Decimator::new(down_sampling_factor),
            render_ds: vec![0.0; sub_block_size],
            buffer_headroom: config.filter.refined.length_blocks,
            capture_call_counter: 0,
            render_call_counter: 0,
            external_audio_buffer_delay: None,
            min_latency_blocks: 0,
            excess_render_detection_counter: 0,
        };
        buffer.reset();
        buffer
    }

    pub(crate) fn reset(&mut self) {
        self.min_latency_blocks = 0;
        self.excess_render_detection_counter = 0;
        self.low_rate.read = self
            .low_rate
            .offset(self.low_rate.write, self.sub_block_size as isize);
        if let Some(external) = self.external_audio_buffer_delay {
            const HEADROOM: i32 = 2;
            let delay_to_set = if external <= HEADROOM {
                1
            } else {
                (external - HEADROOM) as usize
            };
            self.apply_total_delay(delay_to_set.min(self.max_delay()) as isize);
            self.delay = Some(self.compute_delay() as usize);
        } else {
            self.apply_total_delay(self.default_delay as isize);
            self.delay = None;
        }
    }

    pub(crate) fn insert(&mut self, block: &Block) -> BufferingEvent {
        self.render_call_counter += 1;
        let previous_write = self.render.blocks.write;
        self.increment_write_indices();
        let event = if self.render_overrun() {
            BufferingEvent::RenderOverrun
        } else {
            BufferingEvent::None
        };
        self.insert_block(block, previous_write);
        if event != BufferingEvent::None {
            self.reset();
        }
        event
    }

    pub(crate) fn handle_skipped_capture_processing(&mut self) {
        self.capture_call_counter += 1;
    }

    pub(crate) fn prepare_capture_processing(&mut self) -> BufferingEvent {
        self.capture_call_counter += 1;
        if self.detect_excess_render_blocks() {
            self.reset();
            BufferingEvent::RenderOverrun
        } else if self.render_underrun() {
            self.increment_read_indices();
            if let Some(delay) = self.delay.filter(|delay| *delay > 0) {
                self.delay = Some(delay - 1);
            }
            BufferingEvent::RenderUnderrun
        } else {
            self.low_rate.update_read(-(self.sub_block_size as isize));
            self.increment_read_indices();
            BufferingEvent::None
        }
    }

    pub(crate) fn align_from_delay(&mut self, delay: usize) -> bool {
        if self.delay == Some(delay) {
            return false;
        }
        self.delay = Some(delay);
        let total_delay = self.buffer_latency() + delay as isize;
        self.apply_total_delay(self.max_delay().min(total_delay.max(0) as usize) as isize);
        true
    }

    pub(crate) fn align_from_external_delay(&mut self) {
        if let Some(external) = self.external_audio_buffer_delay {
            let delay = self.render_call_counter - self.capture_call_counter + i64::from(external);
            let delay_with_headroom = delay - self.delay_headroom_blocks as i64;
            self.apply_total_delay(delay_with_headroom as i32 as isize);
        }
    }

    pub(crate) fn delay(&self) -> isize {
        self.compute_delay()
    }

    pub(crate) fn render_buffer(&self) -> &RenderBuffer {
        &self.render
    }

    pub(crate) fn downsampled_render_buffer(&self) -> &Ring<f32> {
        &self.low_rate
    }

    pub(crate) fn set_audio_buffer_delay(&mut self, delay_ms: i32) {
        self.external_audio_buffer_delay = Some(delay_ms / 4);
    }

    pub(crate) fn has_received_buffer_delay(&self) -> bool {
        self.external_audio_buffer_delay.is_some()
    }

    fn max_delay(&self) -> usize {
        self.render.blocks.len() - 1 - self.buffer_headroom
    }

    fn compute_delay(&self) -> isize {
        let spectra = &self.render.spectra;
        let internal_delay = if spectra.read >= spectra.write {
            spectra.read - spectra.write
        } else {
            spectra.len() + spectra.read - spectra.write
        };
        internal_delay as isize - self.buffer_latency()
    }

    fn apply_total_delay(&mut self, delay: isize) {
        let render = &mut self.render;
        render.blocks.read = render.blocks.offset(render.blocks.write, -delay);
        render.spectra.read = render.spectra.offset(render.spectra.write, delay);
        render.ffts.read = render.ffts.offset(render.ffts.write, delay);
    }

    fn insert_block(&mut self, block: &Block, previous_write: usize) {
        let render = &mut self.render;
        let write = render.blocks.write;
        let stored = &mut render.blocks.buffer[write];
        for band in 0..stored.bands() {
            for channel in 0..stored.channels() {
                *stored.view_mut(band, channel) = *block.view(band, channel);
            }
        }
        if self.render_linear_amplitude_gain != 1.0 {
            for band in 0..stored.bands() {
                for channel in 0..stored.channels() {
                    for sample in stored.view_mut(band, channel) {
                        *sample *= self.render_linear_amplitude_gain;
                    }
                }
            }
        }

        let mut downmixed = [0.0; BLOCK_LEN];
        self.render_mixer
            .produce_output(&render.blocks.buffer[write], &mut downmixed);
        self.render_decimator
            .decimate(&downmixed, &mut self.render_ds);
        let low_write = self.low_rate.write;
        for (out, sample) in self.low_rate.buffer[low_write..]
            .iter_mut()
            .zip(self.render_ds.iter().rev())
        {
            *out = *sample;
        }

        let (fft_write, spectrum_write) = (render.ffts.write, render.spectra.write);
        for channel in 0..render.blocks.buffer[write].channels() {
            let spectrum = &mut render.ffts.buffer[fft_write][channel];
            fft::padded_fft(
                render.blocks.buffer[write].view(0, channel),
                render.blocks.buffer[previous_write].view(0, channel),
                spectrum,
            );
            spectrum.spectrum(&mut render.spectra.buffer[spectrum_write][channel]);
        }
    }

    fn detect_excess_render_blocks(&mut self) -> bool {
        let latency_blocks = self.buffer_latency() as usize;
        self.min_latency_blocks = self.min_latency_blocks.min(latency_blocks);
        self.excess_render_detection_counter += 1;
        if self.excess_render_detection_counter < self.excess_render_detection_interval_blocks {
            return false;
        }
        let excess = self.min_latency_blocks > self.max_allowed_excess_render_blocks;
        self.min_latency_blocks = latency_blocks;
        self.excess_render_detection_counter = 0;
        excess
    }

    fn buffer_latency(&self) -> isize {
        let low = &self.low_rate;
        let latency_samples = (low.len() + low.read - low.write) % low.len();
        (latency_samples / self.sub_block_size) as isize
    }

    fn increment_write_indices(&mut self) {
        self.low_rate.update_write(-(self.sub_block_size as isize));
        self.render.blocks.inc_write();
        self.render.spectra.dec_write();
        self.render.ffts.dec_write();
    }

    fn increment_read_indices(&mut self) {
        let render = &mut self.render;
        if render.blocks.read != render.blocks.write {
            render.blocks.inc_read();
            render.spectra.dec_read();
            render.ffts.dec_read();
        }
    }

    fn render_overrun(&self) -> bool {
        self.low_rate.read == self.low_rate.write
            || self.render.blocks.read == self.render.blocks.write
    }

    fn render_underrun(&self) -> bool {
        self.low_rate.read == self.low_rate.write
    }
}
