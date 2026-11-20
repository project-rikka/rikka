use rikka_apm_audio::BANDS;

use super::{FrameBands, sub_frame_range};
use crate::block::Block;
use crate::common::BLOCK_LEN;

#[derive(Clone, Debug)]
pub(crate) struct FrameBlocker {
    buffer: Vec<Vec<Vec<f32>>>,
}

impl FrameBlocker {
    pub(crate) fn new(channels: usize) -> FrameBlocker {
        FrameBlocker {
            buffer: vec![vec![Vec::with_capacity(BLOCK_LEN); channels]; BANDS],
        }
    }

    pub(crate) fn insert_sub_frame_and_extract_block(
        &mut self,
        frame: &[FrameBands],
        index: usize,
        block: &mut Block,
    ) {
        for (band, buffers) in self.buffer.iter_mut().enumerate() {
            for (channel, buffer) in buffers.iter_mut().enumerate() {
                let input = &frame[channel][band][sub_frame_range(index)];
                let buffered = buffer.len();
                let samples_to_block = BLOCK_LEN - buffered;
                let out = block.view_mut(band, channel);
                out[..buffered].copy_from_slice(buffer);
                out[buffered..].copy_from_slice(&input[..samples_to_block]);
                buffer.clear();
                buffer.extend_from_slice(&input[samples_to_block..]);
            }
        }
    }

    pub(crate) fn is_block_available(&self) -> bool {
        self.buffer[0][0].len() == BLOCK_LEN
    }

    pub(crate) fn extract_block(&mut self, block: &mut Block) {
        for (band, buffers) in self.buffer.iter_mut().enumerate() {
            for (channel, buffer) in buffers.iter_mut().enumerate() {
                block.view_mut(band, channel).copy_from_slice(buffer);
                buffer.clear();
            }
        }
    }
}
