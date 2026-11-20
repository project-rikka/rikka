use rikka_apm_audio::{BAND, BANDS, Frame};

use crate::block::Block;
use crate::block_delay_buffer::BlockDelayBuffer;
use crate::block_processor::BlockProcessor;
use crate::common::SUB_FRAME_LEN;
use crate::config::Config;
use crate::framing::{BlockFramer, FrameBands, FrameBlocker};
use crate::multichannel_detector::MultichannelContentDetector;
use crate::render_queue::RenderQueue;

const SATURATION_LEVEL: f32 = 32_700.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    pub echo_return_loss: f64,
    pub echo_return_loss_enhancement: f64,
    pub delay_ms: i32,
}

#[derive(Clone, Debug)]
pub struct EchoCanceller {
    mono_config: Config,
    multichannel_config: Option<Config>,
    render_input_channels: usize,
    capture_channels: usize,
    multichannel: MultichannelContentDetector,
    render_queue: RenderQueue,
    render_channels_to_aec: usize,
    render_blocker: FrameBlocker,
    capture_blocker: FrameBlocker,
    output_framer: BlockFramer,
    block_processor: BlockProcessor,
    block_delay: Option<BlockDelayBuffer>,
    render_block: Block,
    capture_block: Block,
    render_frame: Vec<FrameBands>,
    capture_frame: Vec<FrameBands>,
    saturated_microphone: bool,
}

impl EchoCanceller {
    #[must_use]
    pub fn new(config: &Config, render_channels: usize, capture_channels: usize) -> EchoCanceller {
        EchoCanceller::with_multichannel_config(
            config,
            Some(&Config::multichannel()),
            render_channels,
            capture_channels,
        )
    }

    #[must_use]
    pub fn with_multichannel_config(
        config: &Config,
        multichannel_config: Option<&Config>,
        render_channels: usize,
        capture_channels: usize,
    ) -> EchoCanceller {
        let render_input_channels = render_channels.max(1);
        let capture_channels = capture_channels.max(1);
        let mut mono_config = config.clone();
        mono_config.validate();
        let multichannel_config = multichannel_config.map(|multichannel| {
            let mut multichannel = multichannel.clone();
            multichannel.validate();
            multichannel
        });
        let multichannel =
            MultichannelContentDetector::new(&mono_config.multi_channel, render_input_channels);
        let proper = multichannel.is_proper_multichannel_content_detected();
        let render_channels_to_aec = if proper { render_input_channels } else { 1 };
        let active = active_config(&mono_config, multichannel_config.as_ref(), proper);
        let block_delay = (active.delay.fixed_capture_delay_samples > 0).then(|| {
            BlockDelayBuffer::new(
                capture_channels,
                BANDS,
                mono_config.delay.fixed_capture_delay_samples,
            )
        });
        let render_queue = RenderQueue::new(
            render_input_channels,
            active.filter.high_pass_filter_echo_reference,
        );
        let block_processor =
            BlockProcessor::new(active, BANDS, render_channels_to_aec, capture_channels);
        EchoCanceller {
            render_input_channels,
            capture_channels,
            multichannel,
            render_queue,
            render_channels_to_aec,
            render_blocker: FrameBlocker::new(render_channels_to_aec),
            capture_blocker: FrameBlocker::new(capture_channels),
            output_framer: BlockFramer::new(capture_channels),
            block_processor,
            block_delay,
            render_block: Block::new(BANDS, render_channels_to_aec),
            capture_block: Block::new(BANDS, capture_channels),
            render_frame: vec![[[0.0; BAND]; BANDS]; render_input_channels],
            capture_frame: vec![[[0.0; BAND]; BANDS]; capture_channels],
            saturated_microphone: false,
            mono_config,
            multichannel_config,
        }
    }

    pub fn render(&mut self, frame: &Frame) {
        assert_eq!(
            frame.channels(),
            self.render_input_channels,
            "render frames carry the configured render channels"
        );
        self.render_queue.insert(frame);
    }

    pub fn analyze_capture(&mut self, frame: &Frame) {
        self.saturated_microphone = (0..frame.channels()).any(|channel| {
            frame
                .full(channel)
                .iter()
                .any(|sample| *sample >= SATURATION_LEVEL || *sample <= -SATURATION_LEVEL)
        });
    }

    pub fn process_capture(&mut self, frame: &mut Frame, echo_path_change: bool) {
        assert_eq!(
            frame.channels(),
            self.capture_channels,
            "capture frames carry the configured capture channels"
        );
        for (channel, bands) in self.capture_frame.iter_mut().enumerate() {
            *bands = *frame.bands(channel);
        }
        if let Some(block_delay) = &mut self.block_delay {
            block_delay.delay_signal(&mut self.capture_frame);
        }
        self.empty_render_queue();

        let gain_change = echo_path_change
            || self
                .multichannel
                .is_temporary_multichannel_content_detected();
        for index in 0..2 {
            self.capture_blocker.insert_sub_frame_and_extract_block(
                &self.capture_frame,
                index,
                &mut self.capture_block,
            );
            self.block_processor.process_capture(
                gain_change,
                self.saturated_microphone,
                &mut self.capture_block,
            );
            self.output_framer.insert_block_and_extract_sub_frame(
                &self.capture_block,
                &mut self.capture_frame,
                index,
            );
        }
        if self.capture_blocker.is_block_available() {
            self.capture_blocker.extract_block(&mut self.capture_block);
            self.block_processor.process_capture(
                gain_change,
                self.saturated_microphone,
                &mut self.capture_block,
            );
            self.output_framer.insert_block(&self.capture_block);
        }

        for (channel, bands) in self.capture_frame.iter().enumerate() {
            *frame.bands_mut(channel) = *bands;
        }
    }

    pub fn set_audio_buffer_delay(&mut self, delay_ms: i32) {
        self.block_processor.set_audio_buffer_delay(delay_ms);
    }

    #[must_use]
    pub fn metrics(&self) -> Metrics {
        let metrics = self.block_processor.metrics();
        Metrics {
            echo_return_loss: metrics.echo.echo_return_loss,
            echo_return_loss_enhancement: metrics.echo.echo_return_loss_enhancement,
            delay_ms: metrics.delay_ms,
        }
    }

    fn initialize(&mut self) {
        let proper = self.multichannel.is_proper_multichannel_content_detected();
        self.render_channels_to_aec = if proper {
            self.render_input_channels
        } else {
            1
        };
        self.render_block.set_channels(self.render_channels_to_aec);
        self.render_blocker = FrameBlocker::new(self.render_channels_to_aec);
        let active = active_config(&self.mono_config, self.multichannel_config.as_ref(), proper);
        self.block_processor = BlockProcessor::new(
            active,
            BANDS,
            self.render_channels_to_aec,
            self.capture_channels,
        );
    }

    fn empty_render_queue(&mut self) {
        while self.render_queue.remove(&mut self.render_frame) {
            if self.multichannel.update_detection(&self.render_frame) {
                self.initialize();
            }
            let downmix = self
                .multichannel
                .is_temporary_multichannel_content_detected();
            for index in 0..2 {
                if self.render_frame.len() > self.render_channels_to_aec && downmix {
                    downmix_sub_frame(&mut self.render_frame, index);
                }
                self.render_blocker.insert_sub_frame_and_extract_block(
                    &self.render_frame,
                    index,
                    &mut self.render_block,
                );
                self.block_processor.buffer_render(&self.render_block);
            }
            if self.render_blocker.is_block_available() {
                self.render_blocker.extract_block(&mut self.render_block);
                self.block_processor.buffer_render(&self.render_block);
            }
        }
    }
}

fn active_config<'a>(
    mono: &'a Config,
    multichannel: Option<&'a Config>,
    multichannel_content: bool,
) -> &'a Config {
    match multichannel {
        Some(multichannel) if multichannel_content => multichannel,
        _ => mono,
    }
}

fn downmix_sub_frame(frame: &mut [FrameBands], index: usize) {
    let range = index * SUB_FRAME_LEN..(index + 1) * SUB_FRAME_LEN;
    let one_by_channels = 1.0 / frame.len() as f32;
    let (first, rest) = frame.split_at_mut(1);
    for band in 0..BANDS {
        for other in rest.iter() {
            for (sum, value) in first[0][band][range.clone()]
                .iter_mut()
                .zip(&other[band][range.clone()])
            {
                *sum += value;
            }
        }
        for sum in &mut first[0][band][range.clone()] {
            *sum *= one_by_channels;
        }
    }
}
