use crate::block::Block;
use crate::common::{BLOCK_LEN_LOG2, BLOCKS_PER_SECOND};
use crate::config::Config;
use crate::delay::echo_path_delay_estimator::EchoPathDelayEstimator;
use crate::delay::estimate::{DelayEstimate, Quality};
use crate::ring::Ring;

#[derive(Clone, Debug)]
pub(crate) struct RenderDelayController {
    hysteresis_limit_blocks: usize,
    delay: Option<DelayEstimate>,
    delay_estimator: EchoPathDelayEstimator,
    delay_samples: Option<DelayEstimate>,
    delay_change_counter: usize,
    last_delay_estimate_quality: Quality,
}

impl RenderDelayController {
    pub(crate) fn new(config: &Config, capture_channels: usize) -> RenderDelayController {
        RenderDelayController {
            hysteresis_limit_blocks: config.delay.hysteresis_limit_blocks as i32 as usize,
            delay: None,
            delay_estimator: EchoPathDelayEstimator::new(config, capture_channels),
            delay_samples: None,
            delay_change_counter: 0,
            last_delay_estimate_quality: Quality::Coarse,
        }
    }

    pub(crate) fn reset(&mut self, reset_delay_confidence: bool) {
        self.delay = None;
        self.delay_samples = None;
        self.delay_estimator.reset(reset_delay_confidence);
        self.delay_change_counter = 0;
        if reset_delay_confidence {
            self.last_delay_estimate_quality = Quality::Coarse;
        }
    }

    pub(crate) fn has_clockdrift(&self) -> bool {
        self.delay_estimator.has_clockdrift()
    }

    pub(crate) fn delay(&mut self, render: &Ring<f32>, capture: &Block) -> Option<DelayEstimate> {
        let estimate = self.delay_estimator.estimate_delay(render, capture);
        if let Some(estimate) = estimate {
            if self
                .delay_samples
                .is_none_or(|samples| samples.delay != estimate.delay)
            {
                self.delay_change_counter = 0;
            }
            if let Some(samples) = &mut self.delay_samples {
                samples.blocks_since_last_change = if samples.delay == estimate.delay {
                    samples.blocks_since_last_change + 1
                } else {
                    0
                };
                samples.blocks_since_last_update = 0;
                samples.delay = estimate.delay;
                samples.quality = estimate.quality;
            } else {
                self.delay_samples = Some(estimate);
            }
        } else if let Some(samples) = &mut self.delay_samples {
            samples.blocks_since_last_change += 1;
            samples.blocks_since_last_update += 1;
        }

        if self.delay_change_counter < 2 * BLOCKS_PER_SECOND {
            self.delay_change_counter += 1;
        }
        if let Some(samples) = self.delay_samples {
            let use_hysteresis = self.last_delay_estimate_quality == Quality::Refined
                && samples.quality == Quality::Refined;
            self.delay = Some(buffer_delay(
                self.delay,
                if use_hysteresis {
                    self.hysteresis_limit_blocks
                } else {
                    0
                },
                samples,
            ));
            self.last_delay_estimate_quality = samples.quality;
        }
        self.delay
    }
}

fn buffer_delay(
    current: Option<DelayEstimate>,
    hysteresis_limit_blocks: usize,
    estimated: DelayEstimate,
) -> DelayEstimate {
    let mut new_delay_blocks = estimated.delay >> BLOCK_LEN_LOG2;
    if let Some(current) = current {
        let current_delay_blocks = current.delay;
        if new_delay_blocks > current_delay_blocks
            && new_delay_blocks <= current_delay_blocks + hysteresis_limit_blocks
        {
            new_delay_blocks = current_delay_blocks;
        }
    }
    DelayEstimate {
        delay: new_delay_blocks,
        ..estimated
    }
}
