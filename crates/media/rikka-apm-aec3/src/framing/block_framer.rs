use rikka_apm_audio::BANDS;

use super::{FrameBands, sub_frame_range};
use crate::block::Block;
use crate::common::{BLOCK_LEN, SUB_FRAME_LEN};

#[derive(Clone, Debug)]
pub(crate) struct BlockFramer {
    buffer: Vec<Vec<Vec<f32>>>,
}

impl BlockFramer {
    pub(crate) fn new(channels: usize) -> BlockFramer {
        BlockFramer {
            buffer: vec![vec![vec![0.0; BLOCK_LEN]; channels]; BANDS],
        }
    }

    pub(crate) fn insert_block(&mut self, block: &Block) {
        for (band, buffers) in self.buffer.iter_mut().enumerate() {
            for (channel, buffer) in buffers.iter_mut().enumerate() {
                buffer.splice(0..0, block.view(band, channel).iter().copied());
            }
        }
    }

    pub(crate) fn insert_block_and_extract_sub_frame(
        &mut self,
        block: &Block,
        frame: &mut [FrameBands],
        index: usize,
    ) {
        for (band, buffers) in self.buffer.iter_mut().enumerate() {
            for (channel, buffer) in buffers.iter_mut().enumerate() {
                let input = block.view(band, channel);
                let out = &mut frame[channel][band][sub_frame_range(index)];
                let buffered = buffer.len();
                let samples_to_frame = SUB_FRAME_LEN - buffered;
                out[..buffered].copy_from_slice(buffer);
                out[buffered..].copy_from_slice(&input[..samples_to_frame]);
                buffer.clear();
                buffer.extend_from_slice(&input[samples_to_frame..]);
            }
        }
    }
}
