use crate::block::Block;
use crate::common::{BLOCK_LEN, BLOCKS_PER_SECOND};
use crate::config::AlignmentMixing;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Variant {
    Downmix,
    Adaptive,
    Fixed,
}

#[derive(Clone, Debug)]
pub(crate) struct AlignmentMixer {
    channels: usize,
    one_by_channels: f32,
    excitation_energy_threshold: f32,
    prefer_first_two_channels: bool,
    variant: Variant,
    strong_block_counters: [usize; 2],
    cumulative_energies: Vec<f32>,
    selected_channel: usize,
    block_counter: usize,
}

impl AlignmentMixer {
    pub(crate) fn new(channels: usize, config: &AlignmentMixing) -> AlignmentMixer {
        let variant = if channels == 1 {
            Variant::Fixed
        } else if config.downmix {
            Variant::Downmix
        } else if config.adaptive_selection {
            Variant::Adaptive
        } else {
            Variant::Fixed
        };
        AlignmentMixer {
            channels,
            one_by_channels: 1.0 / channels as f32,
            excitation_energy_threshold: BLOCK_LEN as f32 * config.activity_power_threshold,
            prefer_first_two_channels: config.prefer_first_two_channels,
            variant,
            strong_block_counters: [0; 2],
            cumulative_energies: if variant == Variant::Adaptive {
                vec![0.0; channels]
            } else {
                Vec::new()
            },
            selected_channel: 0,
            block_counter: 0,
        }
    }

    pub(crate) fn produce_output(&mut self, x: &Block, y: &mut [f32; BLOCK_LEN]) {
        if self.variant == Variant::Downmix {
            self.downmix(x, y);
            return;
        }
        let channel = if self.variant == Variant::Fixed {
            0
        } else {
            self.select_channel(x)
        };
        *y = *x.view(0, channel);
    }

    fn downmix(&self, x: &Block, y: &mut [f32; BLOCK_LEN]) {
        *y = *x.view(0, 0);
        for channel in 1..self.channels {
            for (out, sample) in y.iter_mut().zip(x.view(0, channel)) {
                *out += sample;
            }
        }
        for out in y.iter_mut() {
            *out *= self.one_by_channels;
        }
    }

    fn select_channel(&mut self, x: &Block) -> usize {
        const BLOCKS_TO_CHOOSE_LEFT_OR_RIGHT: usize = BLOCKS_PER_SECOND / 2;
        const BLOCKS_BEFORE_ENERGY_SMOOTHING: usize = 60 * BLOCKS_PER_SECOND;
        let good_signal_in_left_or_right = self.prefer_first_two_channels
            && (self.strong_block_counters[0] > BLOCKS_TO_CHOOSE_LEFT_OR_RIGHT
                || self.strong_block_counters[1] > BLOCKS_TO_CHOOSE_LEFT_OR_RIGHT);
        let channels_to_analyze = if good_signal_in_left_or_right {
            2
        } else {
            self.channels
        };

        self.block_counter += 1;
        for channel in 0..channels_to_analyze {
            let energy = crate::common::sum_of_squares(x.view(0, channel));
            if channel < 2 && energy > self.excitation_energy_threshold {
                self.strong_block_counters[channel] += 1;
            }
            let cumulative = &mut self.cumulative_energies[channel];
            if self.block_counter <= BLOCKS_BEFORE_ENERGY_SMOOTHING {
                *cumulative += energy;
            } else {
                const SMOOTHING: f32 = 1.0 / (10 * BLOCKS_PER_SECOND) as f32;
                *cumulative += SMOOTHING * (energy - *cumulative);
            }
        }
        if self.block_counter == BLOCKS_BEFORE_ENERGY_SMOOTHING {
            const ONE_BY_BLOCKS: f32 = 1.0 / BLOCKS_BEFORE_ENERGY_SMOOTHING as f32;
            for cumulative in &mut self.cumulative_energies[..channels_to_analyze] {
                *cumulative *= ONE_BY_BLOCKS;
            }
        }

        let mut strongest = 0;
        for channel in 0..channels_to_analyze {
            if self.cumulative_energies[channel] > self.cumulative_energies[strongest] {
                strongest = channel;
            }
        }
        if (good_signal_in_left_or_right && self.selected_channel > 1)
            || self.cumulative_energies[strongest]
                > 2.0 * self.cumulative_energies[self.selected_channel]
        {
            self.selected_channel = strongest;
        }
        self.selected_channel
    }
}
