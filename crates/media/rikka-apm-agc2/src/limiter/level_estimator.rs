use rikka_apm_audio::{FRAME, Frame};

use crate::common::SUB_FRAMES_IN_FRAME;

const SUB_FRAME: usize = FRAME / SUB_FRAMES_IN_FRAME;
const ATTACK: f32 = 0.0;
const DECAY: f32 = 0.997_125_9;

#[derive(Clone, Debug)]
pub(crate) struct LevelEstimator {
    filter_state: f32,
}

impl LevelEstimator {
    pub(crate) fn new() -> LevelEstimator {
        LevelEstimator { filter_state: 0.0 }
    }

    pub(crate) fn last_level(&self) -> f32 {
        self.filter_state
    }

    pub(crate) fn compute(&mut self, frame: &Frame, channels: usize) -> [f32; SUB_FRAMES_IN_FRAME] {
        let mut envelope = [0.0f32; SUB_FRAMES_IN_FRAME];
        for channel in 0..channels {
            for (level, samples) in envelope
                .iter_mut()
                .zip(frame.full(channel).as_chunks::<SUB_FRAME>().0)
            {
                for sample in samples {
                    *level = level.max(sample.abs());
                }
            }
        }
        for sub_frame in 0..SUB_FRAMES_IN_FRAME - 1 {
            if envelope[sub_frame] < envelope[sub_frame + 1] {
                envelope[sub_frame] = envelope[sub_frame + 1];
            }
        }
        for level in &mut envelope {
            *level = if *level > self.filter_state {
                *level * (1.0 - ATTACK) + self.filter_state * ATTACK
            } else {
                *level * (1.0 - DECAY) + self.filter_state * DECAY
            };
            self.filter_state = *level;
        }
        envelope
    }
}
