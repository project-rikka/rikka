mod block_framer;
mod frame_blocker;

pub(crate) use block_framer::BlockFramer;
pub(crate) use frame_blocker::FrameBlocker;

use rikka_apm_audio::{BAND, BANDS};

use crate::common::SUB_FRAME_LEN;

pub(crate) type FrameBands = [[f32; BAND]; BANDS];

fn sub_frame_range(index: usize) -> std::ops::Range<usize> {
    index * SUB_FRAME_LEN..(index + 1) * SUB_FRAME_LEN
}
