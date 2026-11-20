use crate::block::Block;
use crate::config::Config;
use crate::delay::{DelayEstimate, RenderDelayController};
use crate::echo_path_variability::{DelayAdjustment, EchoPathVariability};
use crate::echo_remover::{EchoMetrics, EchoRemover};
use crate::render::{BufferingEvent, RenderDelayBuffer};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BlockMetrics {
    pub(crate) echo: EchoMetrics,
    pub(crate) delay_ms: i32,
}

#[derive(Clone, Debug)]
pub(crate) struct BlockProcessor {
    capture_properly_started: bool,
    render_properly_started: bool,
    render_buffer: RenderDelayBuffer,
    delay_controller: Option<RenderDelayController>,
    echo_remover: EchoRemover,
    render_event: BufferingEvent,
    estimated_delay: Option<DelayEstimate>,
}

impl BlockProcessor {
    pub(crate) fn new(
        config: &Config,
        bands: usize,
        render_channels: usize,
        capture_channels: usize,
    ) -> BlockProcessor {
        BlockProcessor {
            capture_properly_started: false,
            render_properly_started: false,
            render_buffer: RenderDelayBuffer::new(config, bands, render_channels),
            delay_controller: (!config.delay.use_external_delay_estimator)
                .then(|| RenderDelayController::new(config, capture_channels)),
            echo_remover: EchoRemover::new(config, bands, render_channels, capture_channels),
            render_event: BufferingEvent::None,
            estimated_delay: None,
        }
    }

    pub(crate) fn metrics(&self) -> BlockMetrics {
        const BLOCK_SIZE_MS: i32 = 4;
        BlockMetrics {
            echo: self.echo_remover.metrics(),
            delay_ms: self.render_buffer.delay() as i32 * BLOCK_SIZE_MS,
        }
    }

    pub(crate) fn set_audio_buffer_delay(&mut self, delay_ms: i32) {
        self.render_buffer.set_audio_buffer_delay(delay_ms);
    }

    pub(crate) fn buffer_render(&mut self, block: &Block) {
        self.render_event = self.render_buffer.insert(block);
        self.render_properly_started = true;
    }

    pub(crate) fn process_capture(
        &mut self,
        echo_path_gain_change: bool,
        capture_signal_saturation: bool,
        capture: &mut Block,
    ) {
        if !self.render_properly_started {
            self.render_buffer.handle_skipped_capture_processing();
            return;
        }
        if !self.capture_properly_started {
            self.capture_properly_started = true;
            self.render_buffer.reset();
            if let Some(controller) = &mut self.delay_controller {
                controller.reset(true);
            }
        }

        let mut variability = EchoPathVariability {
            gain_change: echo_path_gain_change,
            delay_change: DelayAdjustment::None,
            clock_drift: false,
        };
        if self.render_event == BufferingEvent::RenderOverrun {
            variability.delay_change = DelayAdjustment::BufferFlush;
            if let Some(controller) = &mut self.delay_controller {
                controller.reset(true);
            }
        }
        self.render_event = BufferingEvent::None;

        if self.render_buffer.prepare_capture_processing() == BufferingEvent::RenderUnderrun
            && let Some(controller) = &mut self.delay_controller
        {
            controller.reset(false);
        }

        if let Some(controller) = &mut self.delay_controller {
            self.estimated_delay =
                controller.delay(self.render_buffer.downsampled_render_buffer(), capture);
            if let Some(estimate) = self.estimated_delay
                && self.render_buffer.align_from_delay(estimate.delay)
            {
                variability.delay_change = DelayAdjustment::NewDetectedDelay;
            }
            variability.clock_drift = controller.has_clockdrift();
        } else {
            self.render_buffer.align_from_external_delay();
        }

        if self.delay_controller.is_some() || self.render_buffer.has_received_buffer_delay() {
            self.echo_remover.process_capture(
                variability,
                capture_signal_saturation,
                self.estimated_delay,
                self.render_buffer.render_buffer(),
                capture,
            );
        }
    }
}
